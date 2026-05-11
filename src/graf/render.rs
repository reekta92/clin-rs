use std::collections::{HashMap, HashSet};

use fdg_sim::petgraph::graph::NodeIndex;
use fdg_sim::petgraph::visit::{EdgeRef, IntoEdgeReferences};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::canvas::{Canvas, Line, Painter, Shape};

use crate::config::{
    ClinConfig, EdgeColorMode, LabelMode, LegendPosition, NodeColorMode, NodeShape, NodeSizeMode,
};
use crate::graf::graph::GraphState;
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
    let idx = (count as f64 / max_count as f64 * (colors.len() - 1) as f64) as usize;
    colors.get(idx).copied().unwrap_or(Color::Gray)
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

struct GraphEdgesShape {
    edges: Vec<EdgeData>,
}

impl Shape for GraphEdgesShape {
    fn draw(&self, painter: &mut Painter) {
        for edge in &self.edges {
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
    pub selection_ring_color: Color,
    pub shape: NodeShape,
}

struct GraphNodesShape {
    nodes: Vec<NodeRenderData>,
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

impl Shape for GraphNodesShape {
    fn draw(&self, painter: &mut Painter) {
        for node in &self.nodes {
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
    pub x: f64,
    pub y: f64,
    pub text: String,
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
            let color = match config.visual.node_color_mode {
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
            let items = match config.visual.node_color_mode {
                NodeColorMode::Folder => &self.folder_colors,
                _ => &self.tag_colors,
            };
            if items.is_empty() {
                None
            } else {
                let mut sorted: Vec<_> = items.iter().collect();
                sorted.sort_by_key(|(t, _)| t.as_str());
                sorted.truncate(config.legend.max_items);
                Some(sorted.into_iter().map(|(t, c)| (t.clone(), *c)).collect())
            }
        } else {
            None
        };

        self.topology_dirty = false;
    }

    pub fn fill_edges(
        &mut self,
        graph: &fdg_sim::ForceGraph<super::graph::GraphNodeData, ()>,
        config: &ClinConfig,
        edge_color: Color,
    ) {
        self.edges.clear();
        for edge in graph.edge_references() {
            let src = &graph[edge.source()];
            let tgt = &graph[edge.target()];
            let color = match config.visual.edge_color_mode {
                EdgeColorMode::Source => *self
                    .node_own_color
                    .get(&edge.source())
                    .unwrap_or(&edge_color),
                EdgeColorMode::Target => *self
                    .node_own_color
                    .get(&edge.target())
                    .unwrap_or(&edge_color),
                EdgeColorMode::Uniform => edge_color,
            };
            self.edges.push(EdgeData {
                x1: src.location.x as f64,
                y1: src.location.y as f64,
                x2: tgt.location.x as f64,
                y2: tgt.location.y as f64,
                color,
                thickness: config.visual.edge_thickness,
            });
        }
    }

    pub fn fill_nodes(
        &mut self,
        graph: &fdg_sim::ForceGraph<super::graph::GraphNodeData, ()>,
        config: &ClinConfig,
        selected_node: Option<NodeIndex>,
        selection_ring_color: Color,
    ) {
        self.nodes.clear();
        for idx in graph.node_indices() {
            let node = &graph[idx];
            let primary_color = self
                .node_own_color
                .get(&idx)
                .copied()
                .unwrap_or(Color::Gray);
            let radius = match config.visual.node_size_mode {
                NodeSizeMode::Fixed => config.visual.node_size,
                NodeSizeMode::LinkCount => {
                    if self.max_link_count == 0 {
                        config.visual.node_size
                    } else {
                        config.visual.node_size
                            * (1.0
                                + (node.data.link_count as f64 / self.max_link_count as f64) * 1.5)
                    }
                }
            };
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
                is_selected: selected_node == Some(idx),
                selection_ring_color,
                shape: config.visual.node_shape,
            });
        }
    }

    pub fn fill_labels(
        &mut self,
        graph: &fdg_sim::ForceGraph<super::graph::GraphNodeData, ()>,
        config: &ClinConfig,
        selected_node: Option<NodeIndex>,
    ) {
        self.labels.clear();
        let should_show = |idx: NodeIndex| -> bool {
            match config.visual.label_mode {
                LabelMode::Selected => selected_node == Some(idx),
                LabelMode::Neighbors => {
                    if selected_node == Some(idx) {
                        return true;
                    }
                    if let Some(sel) = selected_node {
                        for edge in graph.edges(sel) {
                            if edge.target() == idx || edge.source() == idx {
                                return true;
                            }
                        }
                    }
                    false
                }
                LabelMode::All => true,
                LabelMode::None => false,
            }
        };

        for idx in graph.node_indices() {
            if !should_show(idx) {
                continue;
            }
            let node = &graph[idx];
            let radius = self.nodes.get(idx.index()).map(|n| n.radius).unwrap_or(2.0);
            self.labels.push(LabelData {
                x: node.location.x as f64,
                y: node.location.y as f64 + radius + config.visual.label_offset,
                text: crate::graf::util::truncate(&node.data.title, config.visual.label_max_length),
            });
        }
    }
}

pub fn draw_graph_view(
    frame: &mut ratatui::Frame,
    area: Rect,
    state: &GraphState,
    config: &ClinConfig,
    flags: &FeatureFlags,
) {
    let aspect = area.width as f64 / area.height as f64;
    let viewport = &state.viewport;
    let colors = config.theme_colors();
    let graph = state.simulation.get_graph();

    let mut cache = state.render_cache.lock().unwrap_or_else(|e| e.into_inner());

    if cache.topology_dirty || (flags.show_legend && cache.legend_data.is_none()) {
        cache.rebuild_topology(graph, config, &colors, flags.show_legend);
    }

    cache.fill_edges(graph, config, colors.edge_color);
    cache.fill_nodes(
        graph,
        config,
        state.selected_node,
        colors.selected_indicator_color,
    );
    cache.fill_labels(graph, config, state.selected_node);

    let edges = cache.edges.clone();
    let nodes = cache.nodes.clone();
    let labels = cache.labels.clone();

    let node_count = graph.node_count();
    let edge_count = graph.edge_count();

    let x_bounds = viewport.x_bounds(aspect);
    let y_bounds = viewport.y_bounds(aspect);

    let block = ratatui::widgets::Block::default().style(
        ratatui::style::Style::default().bg(colors.background_color.unwrap_or(Color::Reset)),
    );

    let canvas = Canvas::default()
        .background_color(colors.background_color.unwrap_or(Color::Reset))
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .block(block)
        .marker(ratatui::symbols::Marker::from(
            config.visual.canvas_marker.clone(),
        ))
        .paint(move |ctx| {
            if flags.show_grid {
                draw_grid(
                    ctx,
                    x_bounds,
                    y_bounds,
                    colors.grid_color,
                    config.visual.grid_divisions,
                );
            }
            ctx.draw(&GraphEdgesShape {
                edges: edges.clone(),
            });
            ctx.layer();
            ctx.draw(&GraphNodesShape {
                nodes: nodes.clone(),
            });
            ctx.layer();
            for label in &labels {
                let span = ratatui::text::Span::styled(
                    label.text.clone(),
                    ratatui::style::Style::default().fg(colors.label_color),
                );
                ctx.print(label.x, label.y, span);
            }
        });

    frame.render_widget(canvas, area);

    if flags.show_legend
        && let Some(ref items) = cache.legend_data
    {
        let max_len = items
            .iter()
            .map(|(t, _): &(String, ratatui::style::Color)| t.len())
            .max()
            .unwrap_or(0);
        let legend_width = (max_len + 4) as u16;
        let legend_height = (items.len() as u16).min(config.legend.max_items as u16) + 2;
        let (legend_x, legend_y) = match config.legend.position {
            LegendPosition::TopLeft => (area.x, area.y),
            LegendPosition::TopRight => (area.x + area.width.saturating_sub(legend_width), area.y),
            LegendPosition::BottomLeft => (
                area.x,
                area.y + area.height.saturating_sub(legend_height + 1),
            ),
            LegendPosition::BottomRight => (
                area.x + area.width.saturating_sub(legend_width),
                area.y + area.height.saturating_sub(legend_height + 1),
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
        let selected_info = state
            .selected_node
            .and_then(|idx| graph.node_weight(idx))
            .map(|n| n.data.title.clone());

        let (viewport_size_pct, viewport_ratio) = {
            let (gx_min, gx_max, gy_min, gy_max) = state.graph_bounds;
            let graph_w = gx_max - gx_min;
            let graph_h = gy_max - gy_min;
            let vp_w = x_bounds[1] - x_bounds[0];
            let vp_h = y_bounds[1] - y_bounds[0];
            let graph_area = graph_w * graph_h;
            let vp_area = vp_w * vp_h;
            let size_pct = if graph_area > 0.0 {
                (vp_area / graph_area * 100.0).clamp(0.0, 100.0)
            } else {
                100.0
            };
            let range = graph_w.max(graph_h).max(1.0) * 1.4;
            let full_zoom = 200.0 / range;
            let ratio = viewport.zoom / full_zoom;
            (size_pct, ratio)
        };

        let status = config.expand_status(
            node_count,
            edge_count,
            selected_info.as_deref(),
            Some(viewport_size_pct),
            Some(viewport_ratio),
        );

        let status_bar = ratatui::widgets::Paragraph::new(status)
            .style(ratatui::style::Style::default().fg(colors.status_bar_color));
        let status_area = ratatui::layout::Rect::new(
            area.x,
            area.y + area.height.saturating_sub(1),
            area.width,
            1,
        );
        frame.render_widget(status_bar, status_area);
    }

    if flags.show_minimap {
        let minimap_area = compute_minimap_area(area, config);

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
        );

        cache.minimap_grid = minimap_grid;
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
    let w = config.visual.minimap_width;
    let h = config.visual.minimap_height;
    let (x, y) = match config.visual.minimap_position {
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

    let grid_size = sub_h * iw;
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
