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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrafMenuType {
    Node,
    MultiNode,
    Background,
}

#[derive(Debug, Clone)]
pub struct GrafContextMenu {
    /// screen-col offset within graph area
    pub x: u16,
    /// screen-row offset within graph area
    pub y: u16,
    pub selected: usize,
    pub items: Vec<GrafMenuItem>,
    pub menu_type: GrafMenuType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrafMenuItem {
    CreateConnection,
    DeleteConnection,
    LocalGraph,
    ShowGroup,
    DeleteNode,
}

pub fn menu_item_shortcut_char(item: GrafMenuItem) -> char {
    match item {
        GrafMenuItem::CreateConnection => 'c',
        GrafMenuItem::DeleteConnection => 'd',
        GrafMenuItem::LocalGraph => 'l',
        GrafMenuItem::ShowGroup => 'g',
        GrafMenuItem::DeleteNode => 'x',
    }
}

pub fn menu_item_label(item: GrafMenuItem) -> &'static str {
    match item {
        GrafMenuItem::CreateConnection => "Create Connection",
        GrafMenuItem::DeleteConnection => "Delete Connection",
        GrafMenuItem::LocalGraph => "Local Graph",
        GrafMenuItem::ShowGroup => "Show Group",
        GrafMenuItem::DeleteNode => "Delete Node",
    }
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
    pub selected_node: Option<NodeIndex>,
    pub selected_nodes: HashSet<NodeIndex>,
    pub dragging_node: Option<NodeIndex>,
    pub drag_target: Option<(f32, f32)>,
    pub is_settled: bool,
    pub alpha: f32,
    pub graph_bounds: (f64, f64, f64, f64),
    pub render_cache: Mutex<super::render::RenderCache>,
    pub mouse_pos: Option<(u16, u16)>,
    pub spatial_grid: super::spatial::SpatialGrid,
    pub physics_worker_active: bool,
    pub physics_ideal_distance: f64,
    pub context_menu: Option<GrafContextMenu>,
    pub context_menu_screen: (u16, u16),
    pub connection_source: Option<NodeIndex>,
    pub deleting_connection_source: Option<NodeIndex>,
    pub box_select_start: Option<(f64, f64)>,
    pub box_select_curr: Option<(f64, f64)>,
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
        crate::config::defaults::default_ideal_distance()
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
            selected_node: None,
            selected_nodes: HashSet::new(),
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            alpha: 0.4,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: Mutex::new(super::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: super::spatial::SpatialGrid::new(config.graf.physics.ideal_distance),
            physics_worker_active: false,
            physics_ideal_distance: config.graf.physics.ideal_distance,
            context_menu: None,
            context_menu_screen: (0, 0),
            connection_source: None,
            deleting_connection_source: None,
            box_select_start: None,
            box_select_curr: None,
            right_down_pos: None,
            mode_banner: None,
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

    pub fn open_context_menu(&mut self, screen_x: u16, screen_y: u16, _world: (f64, f64)) {
        let items = if !self.selected_nodes.is_empty() {
            vec![GrafMenuItem::ShowGroup, GrafMenuItem::DeleteNode]
        } else if self.selected_node.is_some() {
            vec![
                GrafMenuItem::CreateConnection,
                GrafMenuItem::DeleteConnection,
                GrafMenuItem::LocalGraph,
                GrafMenuItem::DeleteNode,
            ]
        } else {
            return;
        };
        self.context_menu = Some(GrafContextMenu {
            x: screen_x,
            y: screen_y,
            selected: 0,
            menu_type: if self.selected_nodes.is_empty() {
                GrafMenuType::Node
            } else {
                GrafMenuType::MultiNode
            },
            items,
        });
        self.context_menu_screen = (screen_x, screen_y);
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
        self.spatial_grid.rebuild(graph_mut);

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
    fn test_static_cluster_layout_geometry() {
        let summaries = vec![
            NoteSummary {
                id: "hub".to_string(),
                title: "hub".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "a".to_string(),
                title: "a".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["leaf_a".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "b".to_string(),
                title: "b".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["leaf_b".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "c".to_string(),
                title: "c".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["leaf_c".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "leaf_a".to_string(),
                title: "leaf_a".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
            NoteSummary {
                id: "leaf_b".to_string(),
                title: "leaf_b".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
            NoteSummary {
                id: "leaf_c".to_string(),
                title: "leaf_c".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
            NoteSummary {
                id: "d".to_string(),
                title: "d".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["e".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "e".to_string(),
                title: "e".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
            NoteSummary {
                id: "isolated".to_string(),
                title: "isolated".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
        ];

        let mut config = ClinConfig::default();
        config.graf.filter.show_orphan = true;
        let spacing = 80.0;
        config.graf.physics.ideal_distance = spacing;

        let mut gs = GraphState::new(&summaries, &config).unwrap();

        let success = gs.apply_static_cluster_layout(spacing);
        assert!(success);

        let graph = gs.simulation.get_graph();
        let mut node_map = HashMap::new();
        for idx in graph.node_indices() {
            let n = &graph[idx];
            node_map.insert(n.data.note_id.clone(), (idx, n.location));
        }

        assert_eq!(node_map.len(), 10);

        let mut comps = collect_static_components(graph);
        assert_eq!(comps.len(), 3);
        assert_eq!(comps[0].nodes.len(), 7);
        assert_eq!(comps[1].nodes.len(), 2);
        assert_eq!(comps[2].nodes.len(), 1);

        assert_eq!(comps[0].key, "a");
        assert_eq!(comps[1].key, "d");
        assert_eq!(comps[2].key, "isolated");

        layout_static_components(&mut comps, spacing);

        let c0 = comps[0].center;
        assert_eq!(c0, (0.0, 0.0));

        // Assert hub is exactly at its component center
        let loc_hub = node_map.get("hub").unwrap().1;
        assert!((loc_hub.x as f64 - c0.0).abs() < 1e-4);
        assert!((loc_hub.y as f64 - c0.1).abs() < 1e-4);

        // All degree-2 nodes have nonzero equal intermediate radius (spacing)
        let loc_a = node_map.get("a").unwrap().1;
        let loc_b = node_map.get("b").unwrap().1;
        let loc_c = node_map.get("c").unwrap().1;
        let r_a = (loc_a.x as f64 - c0.0).hypot(loc_a.y as f64 - c0.1);
        let r_b = (loc_b.x as f64 - c0.0).hypot(loc_b.y as f64 - c0.1);
        let r_c = (loc_c.x as f64 - c0.0).hypot(loc_c.y as f64 - c0.1);
        assert!((r_a - spacing).abs() < 1e-4);
        assert!((r_b - spacing).abs() < 1e-4);
        assert!((r_c - spacing).abs() < 1e-4);

        // All degree-1 nodes occupy a strictly greater radius (2.0 * spacing)
        let loc_la = node_map.get("leaf_a").unwrap().1;
        let loc_lb = node_map.get("leaf_b").unwrap().1;
        let loc_lc = node_map.get("leaf_c").unwrap().1;
        let r_la = (loc_la.x as f64 - c0.0).hypot(loc_la.y as f64 - c0.1);
        let r_lb = (loc_lb.x as f64 - c0.0).hypot(loc_lb.y as f64 - c0.1);
        let r_lc = (loc_lc.x as f64 - c0.0).hypot(loc_lc.y as f64 - c0.1);
        assert!((r_la - 2.0 * spacing).abs() < 1e-4);
        assert!((r_lb - 2.0 * spacing).abs() < 1e-4);
        assert!((r_lc - 2.0 * spacing).abs() < 1e-4);

        // No node lies beyond disk_radius
        assert!(comps[0].disk_radius >= r_la - 1e-4);

        // Every pair within component is at least spacing - epsilon
        let comp1_nodes = ["hub", "a", "b", "c", "leaf_a", "leaf_b", "leaf_c"];
        for i in 0..comp1_nodes.len() {
            for j in (i + 1)..comp1_nodes.len() {
                let pos_i = node_map.get(comp1_nodes[i]).unwrap().1;
                let pos_j = node_map.get(comp1_nodes[j]).unwrap().1;
                let d = (pos_i.x - pos_j.x).hypot(pos_i.y - pos_j.y) as f64;
                assert!(
                    d >= spacing - 1e-4,
                    "Pair {}-{} distance {} too small",
                    comp1_nodes[i],
                    comp1_nodes[j],
                    d
                );
            }
        }

        let gap = spacing * 4.0;

        for i in 0..comps.len() {
            for j in (i + 1)..comps.len() {
                let ci = comps[i].center;
                let cj = comps[j].center;
                let dist = (ci.0 - cj.0).hypot(ci.1 - cj.1);
                let min_dist = comps[i].envelope_radius + comps[j].envelope_radius + gap;
                assert!(
                    dist >= min_dist - 1e-4,
                    "Components {} and {} overlap: dist={}, min_dist={}",
                    i,
                    j,
                    dist,
                    min_dist
                );
            }
        }

        // Derive angles around comps[0].center (which is (0,0))
        let angle_a = (loc_a.y as f64).atan2(loc_a.x as f64);
        let angle_b = (loc_b.y as f64).atan2(loc_b.x as f64);
        let angle_c = (loc_c.y as f64).atan2(loc_c.x as f64);

        // Normalize angles to [0, 2PI)
        let norm_angle = |ang: f64| {
            let mut a = ang % (2.0 * std::f64::consts::PI);
            if a < 0.0 {
                a += 2.0 * std::f64::consts::PI;
            }
            a
        };
        let mut angles = vec![
            norm_angle(angle_a),
            norm_angle(angle_b),
            norm_angle(angle_c),
        ];
        angles.sort_by(|x, y| x.partial_cmp(y).unwrap());

        // Successive gaps including wraparound
        let gap0 = angles[1] - angles[0];
        let gap1 = angles[2] - angles[1];
        let gap2 = (angles[0] + 2.0 * std::f64::consts::PI) - angles[2];

        // Assert at least two gaps differ by more than 1e-4
        let diff01 = (gap0 - gap1).abs();
        let diff12 = (gap1 - gap2).abs();
        let diff20 = (gap2 - gap0).abs();
        assert!(
            diff01 > 1e-4 || diff12 > 1e-4 || diff20 > 1e-4,
            "Angular gaps are uniform: {:?}, diffs: {}, {}, {}",
            angles,
            diff01,
            diff12,
            diff20
        );
    }

    #[test]
    fn test_static_cluster_layout_stability_and_validation() {
        // Direct helper assertions
        assert_eq!(
            stable_layout_hash("a", "b", 1, 0),
            0x1eb4_c9ab_64b1_1751_u64
        );
        assert_eq!(
            stable_layout_hash("cluster-a", "node-b", 2, 1),
            0x628b_a93c_5bd0_7ea9_u64
        );
        assert!(
            (stable_layout_unit("a", "b", 1, 0)
                - (0x1eb4_c9ab_64b1_1751_u64 as f64 / u64::MAX as f64))
                .abs()
                < 1e-9
        );

        let summaries_1 = vec![
            NoteSummary {
                id: "hub".to_string(),
                title: "hub".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "a".to_string(),
                title: "a".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["leaf_a".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "b".to_string(),
                title: "b".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["leaf_b".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "c".to_string(),
                title: "c".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["leaf_c".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "leaf_a".to_string(),
                title: "leaf_a".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
            NoteSummary {
                id: "leaf_b".to_string(),
                title: "leaf_b".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
            NoteSummary {
                id: "leaf_c".to_string(),
                title: "leaf_c".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
        ];

        let summaries_2 = vec![
            NoteSummary {
                id: "leaf_c".to_string(),
                title: "leaf_c".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
            NoteSummary {
                id: "leaf_b".to_string(),
                title: "leaf_b".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
            NoteSummary {
                id: "leaf_a".to_string(),
                title: "leaf_a".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec![],
                size_bytes: 0,
            },
            NoteSummary {
                id: "c".to_string(),
                title: "c".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["leaf_c".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "b".to_string(),
                title: "b".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["leaf_b".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "a".to_string(),
                title: "a".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["leaf_a".to_string()],
                size_bytes: 0,
            },
            NoteSummary {
                id: "hub".to_string(),
                title: "hub".to_string(),
                updated_at: 0,
                folder: "".to_string(),
                tags: vec![],
                pinned: false,
                links: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                size_bytes: 0,
            },
        ];

        let config = ClinConfig::default();

        let mut gs1 = GraphState::new(&summaries_1, &config).unwrap();
        gs1.apply_static_cluster_layout(80.0);

        let mut gs2 = GraphState::new(&summaries_2, &config).unwrap();
        gs2.apply_static_cluster_layout(80.0);

        let ids = vec!["hub", "a", "b", "c", "leaf_a", "leaf_b", "leaf_c"];
        for id in ids {
            let loc1 = gs1
                .simulation
                .get_graph()
                .node_weights()
                .find(|n| n.data.note_id == id)
                .unwrap()
                .location;
            let loc2 = gs2
                .simulation
                .get_graph()
                .node_weights()
                .find(|n| n.data.note_id == id)
                .unwrap()
                .location;
            assert!((loc1.x - loc2.x).abs() < 1e-4f32, "Node {} x mismatch", id);
            assert!((loc1.y - loc2.y).abs() < 1e-4f32, "Node {} y mismatch", id);
        }

        // Expect hub at center, and a, b, c exactly spacing away
        let loc_hub = gs1
            .simulation
            .get_graph()
            .node_weights()
            .find(|n| n.data.note_id == "hub")
            .unwrap()
            .location;
        assert!((loc_hub.x as f64).abs() < 1e-4);
        assert!((loc_hub.y as f64).abs() < 1e-4);

        for id in ["a", "b", "c"] {
            let loc = gs1
                .simulation
                .get_graph()
                .node_weights()
                .find(|n| n.data.note_id == id)
                .unwrap()
                .location;
            let dist = (loc.x as f64).hypot(loc.y as f64);
            assert!((dist - 80.0).abs() < 1e-4);
        }

        let spacing_invalid = vec![0.0, -10.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
        for s in spacing_invalid {
            let mut gs = GraphState::new(&summaries_1, &config).unwrap();
            let success = gs.apply_static_cluster_layout(s);
            assert!(success);
            for node in gs.simulation.get_graph().node_weights() {
                assert!(node.location.x.is_finite());
                assert!(node.location.y.is_finite());
            }
        }

        let mut gs_empty = GraphState {
            simulation: fdg_sim::Simulation::from_graph(
                fdg_sim::ForceGraph::default(),
                fdg_sim::SimulationParameters::default(),
            ),
            viewport: crate::graf::viewport::Viewport::default(),
            selected_node: None,
            selected_nodes: HashSet::new(),
            dragging_node: None,
            drag_target: None,
            is_settled: false,
            alpha: 0.4,
            graph_bounds: (0.0, 0.0, 0.0, 0.0),
            render_cache: Mutex::new(crate::graf::render::RenderCache::new()),
            mouse_pos: None,
            spatial_grid: crate::graf::spatial::SpatialGrid::new(100.0),
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
        let success = gs_empty.apply_static_cluster_layout(80.0);
        assert!(!success);
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
        state.selected_node = Some(fdg_sim::petgraph::graph::NodeIndex::new(0));
        state.open_context_menu(0, 0, (0.0, 0.0));
        let menu = state.context_menu.as_ref().unwrap();
        assert_eq!(menu.menu_type, GrafMenuType::Node);
        assert_eq!(menu.items.len(), 4);
        assert!(menu.items.contains(&GrafMenuItem::LocalGraph));
        assert!(!menu.items.contains(&GrafMenuItem::ShowGroup));
        assert!(menu.items.contains(&GrafMenuItem::CreateConnection));
        assert!(menu.items.contains(&GrafMenuItem::DeleteConnection));
        assert!(menu.items.contains(&GrafMenuItem::DeleteNode));

        // MultiNode menu: 2 items, ShowGroup present, LocalGraph absent
        state.context_menu = None;
        state
            .selected_nodes
            .insert(fdg_sim::petgraph::graph::NodeIndex::new(0));
        state.open_context_menu(0, 0, (0.0, 0.0));
        let menu = state.context_menu.as_ref().unwrap();
        assert_eq!(menu.menu_type, GrafMenuType::MultiNode);
        assert_eq!(menu.items.len(), 2);
        assert!(menu.items.contains(&GrafMenuItem::ShowGroup));
        assert!(menu.items.contains(&GrafMenuItem::DeleteNode));
        assert!(!menu.items.contains(&GrafMenuItem::LocalGraph));

        // Background (no selection) → no menu
        state.context_menu = None;
        state.selected_node = None;
        state.selected_nodes.clear();
        state.open_context_menu(0, 0, (0.0, 0.0));
        assert!(state.context_menu.is_none());
    }

    #[test]
    fn test_menu_item_shortcut_round_trip() {
        let all = [
            GrafMenuItem::CreateConnection,
            GrafMenuItem::DeleteConnection,
            GrafMenuItem::LocalGraph,
            GrafMenuItem::ShowGroup,
            GrafMenuItem::DeleteNode,
        ];
        for item in all {
            let c = menu_item_shortcut_char(item);
            assert!(c.is_ascii_lowercase(), "{item:?} shortcut not lowercase");
            assert!(!menu_item_label(item).is_empty());
        }
        // Uniqueness: no two items share a shortcut
        let chars: std::collections::HashSet<char> =
            all.iter().map(|&i| menu_item_shortcut_char(i)).collect();
        assert_eq!(chars.len(), all.len());
    }
}
