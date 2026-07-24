use crate::keybinds::{GraphAction, Keybinds};
use std::collections::{HashMap, HashSet};

use crate::app::ViewMode;
use fdg_sim::petgraph::graph::NodeIndex;
use fdg_sim::petgraph::visit::{EdgeRef, IntoEdgeReferences};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::canvas::{Canvas, Line, Painter, Shape};

use crate::config::{
    ClinConfig, EdgeColorMode, LabelMode, LegendPosition, NodeColorMode, NodeShape, NodeSizeMode,
};
use crate::graf::graph::GraphState;
use crate::graf::spatial::SpatialGrid;
use crate::graf::viewport::Viewport;
fn tag_color(tag: &str, index: usize, _total: usize, palette: &[Color]) -> Color {
    let palette_len = palette.len();
    if palette_len == 0 {
        return Color::Gray;
    }
    let hash = tag
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    palette[((hash as usize) + index * 7) % palette_len]
}

fn link_count_color(count: usize, max_count: usize, colors: &[Color]) -> Color {
    if max_count == 0 {
        return colors.first().copied().unwrap_or(Color::Gray);
    }
    let idx = (count as f64 / max_count as f64 * colors.len().saturating_sub(1) as f64) as usize;
    colors.get(idx).copied().unwrap_or(Color::Gray)
}

/// Level-of-detail tier determined by visible node count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LodTier {
    /// ≤200 visible nodes: full detail (shapes, colors, tag orbits, labels).
    Full,
    /// 201–1000 visible: shapes + colors, no orbits, selected labels only.
    Medium,
    /// >1000 visible: single-pixel dots, no edges, no labels.
    Minimal,
}

#[derive(Clone)]
pub struct EdgeData {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub color: Color,
    pub thickness: u16,
}

struct GraphEdgesShape<'a> {
    edges: &'a [EdgeData],
}

impl Shape for GraphEdgesShape<'_> {
    fn draw(&self, painter: &mut Painter) {
        for edge in self.edges {
            if edge.thickness <= 1 {
                Line {
                    x1: edge.x1,
                    y1: edge.y1,
                    x2: edge.x2,
                    y2: edge.y2,
                    color: edge.color,
                }
                .draw(painter);
            } else {
                let dx = edge.x2 - edge.x1;
                let dy = edge.y2 - edge.y1;
                let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                let nx = -dy / len;
                let ny = dx / len;
                let spacing = 0.4;
                for t in 0..edge.thickness {
                    let offset = (t as f64 - (edge.thickness - 1) as f64 / 2.0) * spacing;
                    Line {
                        x1: edge.x1 + nx * offset,
                        y1: edge.y1 + ny * offset,
                        x2: edge.x2 + nx * offset,
                        y2: edge.y2 + ny * offset,
                        color: edge.color,
                    }
                    .draw(painter);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct NodeRenderData {
    pub x: f64,
    pub y: f64,
    pub color: Color,
    pub radius: f64,
    pub extra_tag_colors: Vec<Color>,
    pub is_selected: bool,
    pub is_hovered: bool,
    pub selection_ring_color: Color,
    pub shape: NodeShape,
}

struct GraphNodesShape<'a> {
    nodes: &'a [NodeRenderData],
}

fn draw_outlined_shape(
    painter: &mut Painter,
    cx: f64,
    cy: f64,
    radius: f64,
    shape: NodeShape,
    color: Color,
) {
    match shape {
        NodeShape::Circle => {
            let steps = 16u32;
            for i in 0..steps {
                let a1 = (i as f64) * std::f64::consts::TAU / (steps as f64);
                let a2 = ((i + 1) as f64) * std::f64::consts::TAU / (steps as f64);
                Line {
                    x1: cx + radius * a1.cos(),
                    y1: cy + radius * a1.sin(),
                    x2: cx + radius * a2.cos(),
                    y2: cy + radius * a2.sin(),
                    color,
                }
                .draw(painter);
            }
        }
        NodeShape::Square => {
            Line {
                x1: cx - radius,
                y1: cy - radius,
                x2: cx + radius,
                y2: cy - radius,
                color,
            }
            .draw(painter);
            Line {
                x1: cx + radius,
                y1: cy - radius,
                x2: cx + radius,
                y2: cy + radius,
                color,
            }
            .draw(painter);
            Line {
                x1: cx + radius,
                y1: cy + radius,
                x2: cx - radius,
                y2: cy + radius,
                color,
            }
            .draw(painter);
            Line {
                x1: cx - radius,
                y1: cy + radius,
                x2: cx - radius,
                y2: cy - radius,
                color,
            }
            .draw(painter);
        }
        NodeShape::Diamond => {
            Line {
                x1: cx,
                y1: cy - radius,
                x2: cx + radius,
                y2: cy,
                color,
            }
            .draw(painter);
            Line {
                x1: cx + radius,
                y1: cy,
                x2: cx,
                y2: cy + radius,
                color,
            }
            .draw(painter);
            Line {
                x1: cx,
                y1: cy + radius,
                x2: cx - radius,
                y2: cy,
                color,
            }
            .draw(painter);
            Line {
                x1: cx - radius,
                y1: cy,
                x2: cx,
                y2: cy - radius,
                color,
            }
            .draw(painter);
        }
    }
}

impl Shape for GraphNodesShape<'_> {
    fn draw(&self, painter: &mut Painter) {
        for node in self.nodes {
            // Draw hover highlight ring (if hovered and not selected)
            if node.is_hovered && !node.is_selected {
                let hover_radius = node.radius + 1.0;
                draw_outlined_shape(
                    painter,
                    node.x,
                    node.y,
                    hover_radius,
                    node.shape,
                    Color::White,
                );
            }

            draw_outlined_shape(painter, node.x, node.y, node.radius, node.shape, node.color);

            let indicator_radius = 1.2;
            let orbit_radius = node.radius + 2.5;
            let extra_count = node.extra_tag_colors.len();
            for (i, &color) in node.extra_tag_colors.iter().enumerate() {
                let angle = (i as f64) * std::f64::consts::TAU / (extra_count as f64)
                    - std::f64::consts::FRAC_PI_2;
                let cx = node.x + orbit_radius * angle.cos();
                let cy = node.y + orbit_radius * angle.sin();
                let dot_steps = 8u32;
                for j in 0..dot_steps {
                    let a1 = (j as f64) * std::f64::consts::TAU / (dot_steps as f64);
                    let a2 = ((j + 1) as f64) * std::f64::consts::TAU / (dot_steps as f64);
                    Line {
                        x1: cx + indicator_radius * a1.cos(),
                        y1: cy + indicator_radius * a1.sin(),
                        x2: cx + indicator_radius * a2.cos(),
                        y2: cy + indicator_radius * a2.sin(),
                        color,
                    }
                    .draw(painter);
                }
            }

            if node.is_selected {
                let ring_radius = node.radius + 1.5;
                draw_outlined_shape(
                    painter,
                    node.x,
                    node.y,
                    ring_radius,
                    node.shape,
                    node.selection_ring_color,
                );
            }
        }
    }
}

#[derive(Clone)]
pub struct LabelData {
    pub node_idx: NodeIndex,
    pub x: f64,
    pub y: f64,
}

pub struct FeatureFlags {
    pub show_legend: bool,
    pub show_grid: bool,
    pub show_minimap: bool,
    pub show_status_bar: bool,
}

pub struct RenderCache {
    pub tag_colors: HashMap<String, Color>,
    pub folder_colors: HashMap<String, Color>,
    pub node_own_color: HashMap<NodeIndex, Color>,
    pub legend_data: Option<Vec<(String, Color)>>,
    pub max_link_count: usize,

    pub edges: Vec<EdgeData>,
    pub nodes: Vec<NodeRenderData>,
    pub labels: Vec<LabelData>,

    pub minimap_grid: Vec<Option<Color>>,

    pub topology_dirty: bool,
    pub minimap_dirty: bool,

    pub visible_nodes: HashSet<NodeIndex>,
    pub selected_neighbors: HashSet<NodeIndex>,
    pub label_texts: HashMap<NodeIndex, String>,
    pub cached_label_max_length: usize,
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderCache {
    pub fn new() -> Self {
        Self {
            tag_colors: HashMap::new(),
            folder_colors: HashMap::new(),
            node_own_color: HashMap::new(),
            legend_data: None,
            max_link_count: 0,
            edges: Vec::new(),
            nodes: Vec::new(),
            labels: Vec::new(),
            minimap_grid: Vec::new(),
            topology_dirty: true,
            minimap_dirty: true,
            visible_nodes: HashSet::new(),
            selected_neighbors: HashSet::new(),
            label_texts: HashMap::new(),
            cached_label_max_length: usize::MAX,
        }
    }

    pub fn rebuild_topology(
        &mut self,
        graph: &fdg_sim::ForceGraph<super::graph::GraphNodeData, ()>,
        config: &ClinConfig,
        colors: &crate::config::ThemeColors,
        show_legend: bool,
    ) {
        self.max_link_count = graph
            .node_weights()
            .map(|n| n.data.link_count)
            .max()
            .unwrap_or(0);

        self.tag_colors.clear();
        {
            let mut unique_tags: HashSet<String> = HashSet::new();
            for node in graph.node_weights() {
                for tag in &node.data.tags {
                    unique_tags.insert(tag.clone());
                }
            }
            let mut sorted_tags: Vec<String> = unique_tags.into_iter().collect();
            sorted_tags.sort();
            let total = sorted_tags.len().max(1);
            for (i, tag) in sorted_tags.into_iter().enumerate() {
                let c = tag_color(&tag, i, total, &colors.node_colors);
                self.tag_colors.insert(tag, c);
            }
        }

        self.folder_colors.clear();
        {
            let mut unique_folders: HashSet<String> = HashSet::new();
            for node in graph.node_weights() {
                unique_folders.insert(node.data.folder.clone());
            }
            let mut sorted_folders: Vec<String> = unique_folders.into_iter().collect();
            sorted_folders.sort();
            let total = sorted_folders.len().max(1);
            for (i, f) in sorted_folders.into_iter().enumerate() {
                let c = tag_color(&f, i, total, &colors.node_colors);
                self.folder_colors.insert(f, c);
            }
        }

        self.node_own_color.clear();
        for idx in graph.node_indices() {
            let node = &graph[idx];
            let color = match config.graf.visual.node_color_mode {
                NodeColorMode::Tag => {
                    if let Some(tag) = node.data.tags.first() {
                        self.tag_colors.get(tag).copied().unwrap_or(Color::Gray)
                    } else {
                        Color::Gray
                    }
                }
                NodeColorMode::Folder => self
                    .folder_colors
                    .get(&node.data.folder)
                    .copied()
                    .unwrap_or(Color::Gray),
                NodeColorMode::LinkCount => link_count_color(
                    node.data.link_count,
                    self.max_link_count,
                    &colors.node_colors,
                ),
                NodeColorMode::Uniform => {
                    colors.node_colors.first().copied().unwrap_or(Color::Gray)
                }
            };
            self.node_own_color.insert(idx, color);
        }

        self.legend_data = if show_legend {
            let items = match config.graf.visual.node_color_mode {
                NodeColorMode::Folder => &self.folder_colors,
                _ => &self.tag_colors,
            };
            if items.is_empty() {
                None
            } else {
                let mut sorted: Vec<_> = items.iter().collect();
                sorted.sort_by_key(|(t, _)| t.as_str());
                sorted.truncate(10);
                Some(sorted.into_iter().map(|(t, c)| (t.clone(), *c)).collect())
            }
        } else {
            None
        };

        self.topology_dirty = false;
        self.label_texts.clear();
        for idx in graph.node_indices() {
            let node = &graph[idx];
            let truncated =
                crate::graf::util::truncate(&node.data.title, config.graf.visual.label_max_length);
            self.label_texts.insert(idx, truncated);
        }
        self.cached_label_max_length = config.graf.visual.label_max_length;
    }

    pub fn fill_edges(
        &mut self,
        graph: &fdg_sim::ForceGraph<super::graph::GraphNodeData, ()>,
        config: &ClinConfig,
        edge_color: Color,
        tier: LodTier,
    ) {
        self.edges.clear();

        if tier == LodTier::Minimal {
            // No edges at minimal LOD
            return;
        }

        let uniform_edges = tier == LodTier::Medium;
        for edge in graph.edge_references() {
            // Skip edge if neither endpoint is visible
            if !self.visible_nodes.contains(&edge.source()) {
                self.visible_nodes.contains(&edge.target());
            }

            let src = &graph[edge.source()];
            let tgt = &graph[edge.target()];
            let color = if uniform_edges {
                edge_color
            } else {
                match config.graf.visual.edge_color_mode {
                    EdgeColorMode::Source => *self
                        .node_own_color
                        .get(&edge.source())
                        .unwrap_or(&edge_color),
                    EdgeColorMode::Target => *self
                        .node_own_color
                        .get(&edge.target())
                        .unwrap_or(&edge_color),
                    EdgeColorMode::Uniform => edge_color,
                }
            };
            self.edges.push(EdgeData {
                x1: src.location.x as f64,
                y1: src.location.y as f64,
                x2: tgt.location.x as f64,
                y2: tgt.location.y as f64,
                color,
                thickness: if uniform_edges {
                    1
                } else {
                    config.graf.visual.edge_thickness
                },
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fill_nodes(
        &mut self,
        graph: &fdg_sim::ForceGraph<super::graph::GraphNodeData, ()>,
        config: &ClinConfig,
        selected_node: Option<NodeIndex>,
        selection_ring_color: Color,
        hovered_node: Option<NodeIndex>,
        spatial_grid: &SpatialGrid,
        x_bounds: [f64; 2],
        y_bounds: [f64; 2],
    ) -> LodTier {
        self.nodes.clear();
        self.visible_nodes.clear();

        spatial_grid.for_each_in_rect(x_bounds[0], y_bounds[0], x_bounds[1], y_bounds[1], |idx| {
            self.visible_nodes.insert(idx);
        });

        // Always include selected node even if off-screen
        if let Some(sel) = selected_node {
            self.visible_nodes.insert(sel);
        }

        // Determine LOD tier from visible node count
        let tier = match self.visible_nodes.len() {
            0..=200 => LodTier::Full,
            201..=1000 => LodTier::Medium,
            _ => LodTier::Minimal,
        };

        for &idx in &self.visible_nodes {
            let node = &graph[idx];
            let primary_color = self
                .node_own_color
                .get(&idx)
                .copied()
                .unwrap_or(Color::Gray);
            let radius = match config.graf.visual.node_size_mode {
                NodeSizeMode::Fixed => config.graf.visual.node_size,
                NodeSizeMode::LinkCount => {
                    if self.max_link_count == 0 {
                        config.graf.visual.node_size
                    } else {
                        config.graf.visual.node_size
                            * (1.0
                                + (node.data.link_count as f64 / self.max_link_count as f64) * 1.5)
                    }
                }
            };

            let is_selected = selected_node == Some(idx);
            let is_hovered = hovered_node == Some(idx) && !is_selected;

            match tier {
                LodTier::Full => {
                    let extra_tag_colors: Vec<Color> = if node.data.tags.is_empty() {
                        Vec::new()
                    } else {
                        node.data
                            .tags
                            .iter()
                            .skip(1)
                            .filter_map(|tag| self.tag_colors.get(tag).copied())
                            .collect()
                    };
                    self.nodes.push(NodeRenderData {
                        x: node.location.x as f64,
                        y: node.location.y as f64,
                        color: primary_color,
                        radius,
                        extra_tag_colors,
                        is_selected,
                        is_hovered,
                        selection_ring_color,
                        shape: config.graf.visual.node_shape,
                    });
                }
                LodTier::Medium => {
                    // No tag orbits, only selected node gets hover ring
                    self.nodes.push(NodeRenderData {
                        x: node.location.x as f64,
                        y: node.location.y as f64,
                        color: primary_color,
                        radius,
                        extra_tag_colors: Vec::new(),
                        is_selected,
                        is_hovered: false,
                        selection_ring_color,
                        shape: config.graf.visual.node_shape,
                    });
                }
                LodTier::Minimal => {
                    // Single-pixel dots, forced Circle shape
                    self.nodes.push(NodeRenderData {
                        x: node.location.x as f64,
                        y: node.location.y as f64,
                        color: primary_color,
                        radius: 1.0,
                        extra_tag_colors: Vec::new(),
                        is_selected,
                        is_hovered: false,
                        selection_ring_color,
                        shape: NodeShape::Circle,
                    });
                }
            }
        }

        tier
    }
    pub fn fill_labels(
        &mut self,
        graph: &fdg_sim::ForceGraph<super::graph::GraphNodeData, ()>,
        config: &ClinConfig,
        selected_node: Option<NodeIndex>,
        min_offset_y: f64,
        tier: LodTier,
    ) {
        self.labels.clear();

        if self.cached_label_max_length != config.graf.visual.label_max_length {
            self.label_texts.clear();
            for idx in graph.node_indices() {
                let node = &graph[idx];
                let truncated = crate::graf::util::truncate(
                    &node.data.title,
                    config.graf.visual.label_max_length,
                );
                self.label_texts.insert(idx, truncated);
            }
            self.cached_label_max_length = config.graf.visual.label_max_length;
        }

        match tier {
            LodTier::Minimal => return,
            LodTier::Medium => {
                if let Some(sel) = selected_node
                    && self.visible_nodes.contains(&sel)
                {
                    let node = &graph[sel];
                    let radius = self.nodes.get(sel.index()).map(|n| n.radius).unwrap_or(2.0);
                    self.labels.push(LabelData {
                        node_idx: sel,
                        x: node.location.x as f64,
                        y: node.location.y as f64
                            + radius
                            + config.graf.visual.label_offset.max(min_offset_y),
                    });
                }
                return;
            }
            LodTier::Full => {}
        }

        self.selected_neighbors.clear();
        if let Some(sel) = selected_node
            && config.graf.visual.label_mode == LabelMode::Neighbors
        {
            for edge in graph.edges(sel) {
                if edge.target() != sel {
                    self.selected_neighbors.insert(edge.target());
                }
                if edge.source() != sel {
                    self.selected_neighbors.insert(edge.source());
                }
            }
        }

        let should_show = |idx: NodeIndex| -> bool {
            match config.graf.visual.label_mode {
                LabelMode::Selected => selected_node == Some(idx),
                LabelMode::Neighbors => {
                    selected_node == Some(idx) || self.selected_neighbors.contains(&idx)
                }
                LabelMode::All => true,
                LabelMode::None => false,
            }
        };

        for &idx in &self.visible_nodes {
            if !should_show(idx) {
                continue;
            }
            let node = &graph[idx];
            let radius = self.nodes.get(idx.index()).map(|n| n.radius).unwrap_or(2.0);
            self.labels.push(LabelData {
                node_idx: idx,
                x: node.location.x as f64,
                y: node.location.y as f64
                    + radius
                    + config.graf.visual.label_offset.max(min_offset_y),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_graph_view(
    frame: &mut ratatui::Frame,
    area: Rect,
    state: &GraphState,
    config: &ClinConfig,
    flags: &FeatureFlags,
    app_theme: &crate::app_theme::AppThemeColors,
    keybinds: &Keybinds,
    pending: Option<&str>,
    mouse_pos: Option<(u16, u16)>,
) {
    let mut canvas_area = area;
    if flags.show_status_bar {
        canvas_area.height = canvas_area.height.saturating_sub(1);
    }
    let aspect = canvas_area.width as f64 / canvas_area.height as f64;
    let viewport = &state.viewport;
    let colors = config.theme_colors();
    let graph = state.simulation.get_graph();

    let mut cache = state.render_cache.lock();

    if cache.topology_dirty || (flags.show_legend && cache.legend_data.is_none()) {
        cache.rebuild_topology(graph, config, &colors, flags.show_legend);
    }

    // Compute hovered node from mouse_pos
    let hovered_node = mouse_pos.and_then(|(col, row)| {
        let (wx, wy) = viewport.screen_to_world(col, row, canvas_area);
        viewport.hit_test(wx, wy, state)
    });

    let x_bounds = viewport.x_bounds(aspect);
    let y_bounds = viewport.y_bounds(aspect);

    let tier = cache.fill_nodes(
        graph,
        config,
        state.selected_node,
        colors.selected_indicator_color,
        hovered_node,
        &state.spatial_grid,
        x_bounds,
        y_bounds,
    );
    cache.fill_edges(graph, config, colors.edge_color, tier);
    let cell_world_height =
        (y_bounds[1] - y_bounds[0]).abs() / (canvas_area.height as f64).max(1.0);
    cache.fill_labels(
        graph,
        config,
        state.selected_node,
        cell_world_height * 1.5,
        tier,
    );
    let edges_ref = &cache.edges;
    let nodes_ref = &cache.nodes;
    let labels_ref = &cache.labels;
    let label_texts_ref = &cache.label_texts;

    let block = ratatui::widgets::Block::default().style(
        ratatui::style::Style::default().bg(colors.background_color.unwrap_or(Color::Reset)),
    );

    let canvas = Canvas::default()
        .background_color(colors.background_color.unwrap_or(Color::Reset))
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .block(block)
        .marker(ratatui::symbols::Marker::from(
            config.graf.visual.canvas_marker,
        ))
        .paint(move |ctx| {
            if flags.show_grid {
                draw_grid(
                    ctx,
                    x_bounds,
                    y_bounds,
                    colors.grid_color,
                    config.graf.visual.grid_divisions,
                );
            }
            ctx.draw(&GraphEdgesShape { edges: edges_ref });
            ctx.layer();
            ctx.draw(&GraphNodesShape { nodes: nodes_ref });
            ctx.layer();
            for label in labels_ref {
                if let Some(text) = label_texts_ref.get(&label.node_idx) {
                    let span = ratatui::text::Span::styled(
                        text.clone(),
                        ratatui::style::Style::default().fg(colors.label_color),
                    );
                    ctx.print(label.x, label.y, span);
                }
            }
        });

    frame.render_widget(canvas, canvas_area);

    if flags.show_legend
        && let Some(ref items) = cache.legend_data
    {
        let max_len = items
            .iter()
            .map(|(t, _): &(String, ratatui::style::Color)| t.len())
            .max()
            .unwrap_or(0);
        let legend_width = (max_len + 4) as u16;
        let legend_height = (items.len() as u16).min(10) + 2;
        let (legend_x, legend_y) = match LegendPosition::BottomRight {
            LegendPosition::TopLeft => (canvas_area.x, canvas_area.y),
            LegendPosition::TopRight => (
                canvas_area.x + canvas_area.width.saturating_sub(legend_width),
                canvas_area.y,
            ),
            LegendPosition::BottomLeft => (
                canvas_area.x,
                canvas_area.y + canvas_area.height.saturating_sub(legend_height + 1),
            ),
            LegendPosition::BottomRight => (
                canvas_area.x + canvas_area.width.saturating_sub(legend_width),
                canvas_area.y + canvas_area.height.saturating_sub(legend_height + 1),
            ),
        };
        let legend_area =
            ratatui::layout::Rect::new(legend_x, legend_y, legend_width, legend_height);
        let legend_text: Vec<ratatui::text::Line> = items
            .iter()
            .map(|(t, c): &(String, ratatui::style::Color)| {
                let display_text = if t.is_empty() { "/" } else { t };
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled("● ", ratatui::style::Style::default().fg(*c)),
                    ratatui::text::Span::styled(
                        display_text,
                        ratatui::style::Style::default().fg(colors.label_color),
                    ),
                ])
            })
            .collect();
        let legend_widget = ratatui::widgets::Paragraph::new(legend_text).block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(ratatui::style::Style::default().fg(colors.border_color)),
        );
        frame.render_widget(legend_widget, legend_area);
    }

    if flags.show_status_bar {
        let status_area = ratatui::layout::Rect::new(
            area.x,
            area.y + area.height.saturating_sub(1),
            area.width,
            1,
        );
        let hints_items = vec![
            (
                format!(
                    "{}/{}",
                    keybinds.display_graph(GraphAction::PanUp),
                    keybinds.display_graph(GraphAction::PanDown)
                ),
                "pan",
            ),
            (
                format!(
                    "{}/{}",
                    keybinds.display_graph(GraphAction::ZoomOut),
                    keybinds.display_graph(GraphAction::ZoomIn)
                ),
                "zoom",
            ),
            (keybinds.display_graph(GraphAction::ToggleLegend), "labels"),
            (keybinds.display_graph(GraphAction::AutoFit), "fit"),
            (keybinds.display_graph(GraphAction::Quit), "quit"),
        ];
        let hint_line = crate::ui::format_keybind_hints(app_theme, &hints_items);
        let mut ctx = crate::statusline::StatuslineContext::for_overlay(config, ViewMode::Graph);
        ctx.area = Some(status_area);
        ctx.graph = Some(state);
        ctx.hints = Some(hint_line.spans);
        if let Some(p) = pending {
            ctx.pending = Some(vec![Span::styled(
                format!("{} ", p),
                Style::default()
                    .fg(app_theme.highlight_fg)
                    .bg(app_theme.accent),
            )]);
        }

        let (left_line, right_line) =
            crate::statusline::render_footer(&ctx, &config.statusline, ViewMode::Graph, app_theme);
        crate::ui::draw_status_bar(frame, status_area, app_theme, left_line, right_line);
    }

    if flags.show_minimap {
        let minimap_area = compute_minimap_area(canvas_area, config);

        // Mark dirty when physics is active (positions may change)
        cache.minimap_dirty = cache.minimap_dirty || !state.is_settled;

        let mut minimap_grid = std::mem::take(&mut cache.minimap_grid);
        draw_minimap(
            frame,
            minimap_area,
            MinimapParams {
                viewport,
                graph,
                graph_bounds: state.graph_bounds,
                node_colors: &cache.node_own_color,
                colors: &colors,
            },
            &mut minimap_grid,
            cache.minimap_dirty,
        );

        cache.minimap_grid = minimap_grid;
        cache.minimap_dirty = false;
    }
}
fn draw_grid(
    ctx: &mut ratatui::widgets::canvas::Context,
    x: [f64; 2],
    y: [f64; 2],
    color: Color,
    divisions: usize,
) {
    let divs = divisions.max(2);
    let step_x = (x[1] - x[0]) / divs as f64;
    let step_y = (y[1] - y[0]) / divs as f64;
    for i in 0..=divs {
        let px = x[0] + step_x * i as f64;
        ctx.draw(&Line {
            x1: px,
            y1: y[0],
            x2: px,
            y2: y[1],
            color,
        });
    }
    for i in 0..=divs {
        let py = y[0] + step_y * i as f64;
        ctx.draw(&Line {
            x1: x[0],
            y1: py,
            x2: x[1],
            y2: py,
            color,
        });
    }
}

pub fn compute_minimap_area(frame_area: Rect, config: &ClinConfig) -> Rect {
    let w = config.graf.visual.minimap_width;
    let h = config.graf.visual.minimap_height;
    let (x, y) = match config.graf.visual.minimap_position {
        LegendPosition::TopLeft => (frame_area.x, frame_area.y),
        LegendPosition::TopRight => (
            frame_area.x + frame_area.width.saturating_sub(w),
            frame_area.y,
        ),
        LegendPosition::BottomLeft => (
            frame_area.x,
            frame_area.y + frame_area.height.saturating_sub(h),
        ),
        LegendPosition::BottomRight => (
            frame_area.x + frame_area.width.saturating_sub(w + 1),
            frame_area.y + frame_area.height.saturating_sub(h + 1),
        ),
    };
    Rect::new(x, y, w, h)
}

pub fn compute_graph_bounds(
    graph: &fdg_sim::ForceGraph<super::graph::GraphNodeData, ()>,
) -> (f64, f64, f64, f64) {
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

    if min_x == f64::MAX {
        min_x = -100.0;
        max_x = 100.0;
        min_y = -100.0;
        max_y = 100.0;
    }

    let pad_x = (max_x - min_x) * 0.1 + 1.0;
    let pad_y = (max_y - min_y) * 0.1 + 1.0;
    (min_x - pad_x, max_x + pad_x, min_y - pad_y, max_y + pad_y)
}

struct MinimapParams<'a> {
    viewport: &'a Viewport,
    graph: &'a fdg_sim::ForceGraph<super::graph::GraphNodeData, ()>,
    graph_bounds: (f64, f64, f64, f64),
    node_colors: &'a HashMap<NodeIndex, Color>,
    colors: &'a crate::config::ThemeColors,
}
fn draw_minimap(
    frame: &mut ratatui::Frame,
    area: Rect,
    params: MinimapParams<'_>,
    grid: &mut Vec<Option<Color>>,
    dirty: bool,
) {
    let (wx_min, wx_max, wy_min, wy_max) = params.graph_bounds;
    let aspect = area.width as f64 / area.height as f64;
    let vp_x = params.viewport.x_bounds(aspect);
    let vp_y = params.viewport.y_bounds(aspect);

    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(params.colors.minimap_border_color))
        .style(ratatui::style::Style::default());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let iw = inner.width as usize;
    let ih = inner.height as usize;
    let sub_h = ih * 2;
    let world_w = wx_max - wx_min;
    let world_h = wy_max - wy_min;

    if world_w <= 0.0 || world_h <= 0.0 {
        return;
    }

    let world_to_col = |x: f64| -> usize {
        let t = (x - wx_min) / world_w;
        let col = (t * iw as f64).floor() as isize;
        col.clamp(0, (iw as isize) - 1) as usize
    };

    let world_to_subrow = |y: f64| -> usize {
        let t = (wy_max - y) / world_h;
        let row = (t * sub_h as f64).floor() as isize;
        row.clamp(0, (sub_h as isize) - 1) as usize
    };

    let world_to_row = |y: f64| -> usize {
        let t = (wy_max - y) / world_h;
        let row = (t * ih as f64).floor() as isize;
        row.clamp(0, (ih as isize) - 1) as usize
    };

    // Only rebuild the pixel grid when dirty (physics changed positions)
    let grid_size = sub_h * iw;
    if dirty || grid.len() != grid_size {
        grid.resize(grid_size, None);
        grid.fill(None);

        for idx in params.graph.node_indices() {
            let node = &params.graph[idx];
            let nx = node.location.x as f64;
            let ny = node.location.y as f64;
            let col = world_to_col(nx);
            let sub_row = world_to_subrow(ny);
            let color = params.node_colors.get(&idx).copied().unwrap_or(Color::Gray);
            grid[sub_row * iw + col] = Some(color);
        }
    }

    let buf = frame.buffer_mut();
    let bg_color: Option<Color> = None;

    for cell_row in 0..ih {
        let top_sub = cell_row * 2;
        let bot_sub = cell_row * 2 + 1;
        for col in 0..iw {
            let top_color = grid[top_sub * iw + col];
            let bot_color = grid[bot_sub * iw + col];

            let x = inner.x + col as u16;
            let y = inner.y + cell_row as u16;

            let cell = match buf.cell_mut((x, y)) {
                Some(c) => c,
                None => continue,
            };

            match (top_color, bot_color) {
                (None, None) => {
                    if let Some(bg) = bg_color {
                        cell.set_symbol(" ");
                        cell.set_style(ratatui::style::Style::default().bg(bg));
                    }
                }
                (Some(tc), None) => {
                    cell.set_symbol("▀");
                    let mut style = ratatui::style::Style::default().fg(tc);
                    if let Some(bg) = bg_color {
                        style = style.bg(bg);
                    }
                    cell.set_style(style);
                }
                (None, Some(bc)) => {
                    cell.set_symbol("▄");
                    let mut style = ratatui::style::Style::default().fg(bc);
                    if let Some(bg) = bg_color {
                        style = style.bg(bg);
                    }
                    cell.set_style(style);
                }
                (Some(tc), Some(bc)) => {
                    cell.set_symbol("▄");
                    cell.set_style(ratatui::style::Style::default().fg(bc).bg(tc));
                }
            }
        }
    }

    let vp_col_min = world_to_col(vp_x[0].max(wx_min));
    let vp_col_max = world_to_col(vp_x[1].min(wx_max));
    let vp_row_min = world_to_row(vp_y[1].min(wy_max));
    let vp_row_max = world_to_row(vp_y[0].max(wy_min));

    if vp_col_min >= vp_col_max || vp_row_min >= vp_row_max {
        return;
    }

    let vp_style = ratatui::style::Style::default().fg(params.colors.minimap_viewport_color);

    for col in vp_col_min..=vp_col_max {
        let x = inner.x + col as u16;

        let y_top = inner.y + vp_row_min as u16;
        if let Some(cell) = buf.cell_mut((x, y_top)) {
            cell.set_symbol("─");
            cell.set_style(vp_style);
        }

        let y_bot = inner.y + vp_row_max as u16;
        if let Some(cell) = buf.cell_mut((x, y_bot)) {
            cell.set_symbol("─");
            cell.set_style(vp_style);
        }
    }

    for row in vp_row_min..=vp_row_max {
        let y = inner.y + row as u16;

        let x_left = inner.x + vp_col_min as u16;
        if let Some(cell) = buf.cell_mut((x_left, y)) {
            cell.set_symbol("│");
            cell.set_style(vp_style);
        }

        let x_right = inner.x + vp_col_max as u16;
        if let Some(cell) = buf.cell_mut((x_right, y)) {
            cell.set_symbol("│");
            cell.set_style(vp_style);
        }
    }

    let corners: [(usize, usize, &str); 4] = [
        (vp_col_min, vp_row_min, "┌"),
        (vp_col_max, vp_row_min, "┐"),
        (vp_col_min, vp_row_max, "└"),
        (vp_col_max, vp_row_max, "┘"),
    ];
    for (col, row, sym) in corners {
        let x = inner.x + col as u16;
        let y = inner.y + row as u16;
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(sym);
            cell.set_style(vp_style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ClinConfig;
    use crate::graf::graph::GraphNodeData;
    use crate::graf::spatial::SpatialGrid;
    use fdg_sim::{ForceGraph, ForceGraphHelper};

    fn setup_spatial_grid(graph: &ForceGraph<GraphNodeData, ()>) -> SpatialGrid {
        let mut grid = SpatialGrid::new(100.0);
        grid.rebuild(graph);
        grid
    }

    // Generous bounds covering all nodes in test graphs
    const TEST_X_BOUNDS: [f64; 2] = [-1000.0, 1000.0];
    const TEST_Y_BOUNDS: [f64; 2] = [-1000.0, 1000.0];

    #[test]
    fn test_fill_labels() {
        let mut graph: ForceGraph<GraphNodeData, ()> = ForceGraph::default();

        let n1_data = GraphNodeData {
            note_id: "1".to_string(),
            title: "Node 1".to_string(),
            tags: vec![],
            link_count: 0,
            folder: "".to_string(),
        };
        let n2_data = GraphNodeData {
            note_id: "2".to_string(),
            title: "Node 2".to_string(),
            tags: vec![],
            link_count: 0,
            folder: "".to_string(),
        };
        let n3_data = GraphNodeData {
            note_id: "3".to_string(),
            title: "Node 3".to_string(),
            tags: vec![],
            link_count: 0,
            folder: "".to_string(),
        };

        let idx1 = graph.add_force_node("Node 1", n1_data);
        let idx2 = graph.add_force_node("Node 2", n2_data);
        let _idx3 = graph.add_force_node("Node 3", n3_data);

        // Add edge: idx1 - idx2 (idx3 is isolated)
        graph.add_edge(idx1, idx2, ());

        let mut cache = RenderCache::new();
        let mut config = ClinConfig::default();
        let grid = setup_spatial_grid(&graph);

        // 1. LabelMode::None
        config.graf.visual.label_mode = crate::config::LabelMode::None;
        let _tier = cache.fill_nodes(
            &graph,
            &config,
            Some(idx1),
            ratatui::style::Color::Red,
            None,
            &grid,
            TEST_X_BOUNDS,
            TEST_Y_BOUNDS,
        );
        cache.fill_labels(&graph, &config, Some(idx1), 0.0, _tier);
        assert!(cache.labels.is_empty());

        // 2. LabelMode::All
        config.graf.visual.label_mode = crate::config::LabelMode::All;
        let _tier = cache.fill_nodes(
            &graph,
            &config,
            Some(idx1),
            ratatui::style::Color::Red,
            None,
            &grid,
            TEST_X_BOUNDS,
            TEST_Y_BOUNDS,
        );
        cache.fill_labels(&graph, &config, Some(idx1), 0.0, _tier);
        assert_eq!(cache.labels.len(), 3);

        // 3. LabelMode::Selected
        config.graf.visual.label_mode = crate::config::LabelMode::Selected;
        let _tier = cache.fill_nodes(
            &graph,
            &config,
            Some(idx1),
            ratatui::style::Color::Red,
            None,
            &grid,
            TEST_X_BOUNDS,
            TEST_Y_BOUNDS,
        );
        cache.fill_labels(&graph, &config, Some(idx1), 0.0, _tier);
        assert_eq!(cache.labels.len(), 1);
        assert_eq!(
            cache.label_texts.get(&cache.labels[0].node_idx).unwrap(),
            "Node 1"
        );

        // 4. LabelMode::Neighbors
        config.graf.visual.label_mode = crate::config::LabelMode::Neighbors;
        let _tier = cache.fill_nodes(
            &graph,
            &config,
            Some(idx1),
            ratatui::style::Color::Red,
            None,
            &grid,
            TEST_X_BOUNDS,
            TEST_Y_BOUNDS,
        );
        cache.fill_labels(&graph, &config, Some(idx1), 0.0, _tier);
        // Node 1 (selected) and Node 2 (neighbor) should have labels. Node 3 (distant) should not.
        assert_eq!(cache.labels.len(), 2);
        let mut names: Vec<String> = cache
            .labels
            .iter()
            .map(|l| cache.label_texts.get(&l.node_idx).unwrap().clone())
            .collect();
        names.sort();
        assert_eq!(names, vec!["Node 1".to_string(), "Node 2".to_string()]);

        // 5. Test min_offset_y parameter
        let _tier = cache.fill_nodes(
            &graph,
            &config,
            Some(idx1),
            ratatui::style::Color::Red,
            None,
            &grid,
            TEST_X_BOUNDS,
            TEST_Y_BOUNDS,
        );
        config.graf.visual.label_mode = crate::config::LabelMode::Selected;
        cache.fill_labels(&graph, &config, Some(idx1), 10.0, _tier);
        assert_eq!(cache.labels.len(), 1);
        let label = &cache.labels[0];
        let node_y = graph[idx1].location.y as f64;
        let radius = cache
            .nodes
            .get(idx1.index())
            .map(|n| n.radius)
            .unwrap_or(2.0);
        // The default label_offset is 4.0, but min_offset_y is 10.0. The actual offset should be 10.0.
        assert_eq!(label.y, node_y + radius + 10.0);

        cache.fill_labels(&graph, &config, Some(idx1), 1.0, _tier);
        let label = &cache.labels[0];
        // The default label_offset is 4.0, which is larger than min_offset_y of 1.0. The actual offset should be 4.0.
        assert_eq!(label.y, node_y + radius + 4.0);
    }
}
