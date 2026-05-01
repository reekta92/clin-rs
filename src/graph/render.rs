use std::collections::{HashMap, HashSet};

use fdg_sim::petgraph::graph::NodeIndex;
use fdg_sim::petgraph::visit::EdgeRef;
use ratatui::style::Color;
use ratatui::widgets::canvas::{Canvas, Line, Painter, Shape};

use super::GraphState;

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let h = h / 360.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h * 6.0) as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    )
}

fn golden_ratio_hash(s: &str) -> f64 {
    let golden = 0.618033988749895;
    let hash: u32 = s
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    (hash as f64 * golden) % 1.0
}

fn tag_color(tag: &str, index: usize, total: usize) -> Color {
    let hue_spread = 360.0 / total as f64;
    let base_hue = (index as f64) * hue_spread;
    let perturbation = golden_ratio_hash(tag) * hue_spread * 0.5 - hue_spread * 0.25;
    let hue = (base_hue + perturbation + 360.0) % 360.0;
    let (r, g, b) = hsl_to_rgb(hue, 0.75, 0.55);
    Color::Rgb(r, g, b)
}

#[derive(Clone)]
struct EdgeData {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

#[derive(Clone)]
struct NodeData {
    x: f64,
    y: f64,
    color: Color,
    extra_tag_colors: Vec<Color>,
    is_selected: bool,
}

struct LabelData {
    x: f64,
    y: f64,
    text: String,
}

struct GraphEdgesData {
    edges: Vec<EdgeData>,
}

impl Shape for GraphEdgesData {
    fn draw(&self, painter: &mut Painter) {
        for edge in &self.edges {
            Line {
                x1: edge.x1,
                y1: edge.y1,
                x2: edge.x2,
                y2: edge.y2,
                color: Color::DarkGray,
            }
            .draw(painter);
        }
    }
}

struct GraphNodesData {
    nodes: Vec<NodeData>,
}

impl Shape for GraphNodesData {
    fn draw(&self, painter: &mut Painter) {
        for node in &self.nodes {
            let radius = if node.is_selected { 3.0 } else { 2.0 };
            let steps = 16u32;
            for i in 0..steps {
                let a1 = (i as f64) * std::f64::consts::TAU / (steps as f64);
                let a2 = ((i + 1) as f64) * std::f64::consts::TAU / (steps as f64);
                Line {
                    x1: node.x + radius * a1.cos(),
                    y1: node.y + radius * a1.sin(),
                    x2: node.x + radius * a2.cos(),
                    y2: node.y + radius * a2.sin(),
                    color: node.color,
                }
                .draw(painter);
            }

            let indicator_radius = 1.2;
            let orbit_radius = radius + 2.5;
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
        }
    }
}

pub fn draw_graph_view(
    frame: &mut ratatui::Frame,
    state: &GraphState,
    label_mode: &crate::config::GraphLabelMode,
) {
    let area = frame.area();
    let aspect = area.width as f64 / area.height as f64;
    let viewport = &state.viewport;

    let tag_colors: HashMap<String, Color> = {
        let graph = state.simulation.get_graph();
        let mut unique_tags: HashSet<String> = HashSet::new();
        for node in graph.node_weights() {
            for tag in &node.data.tags {
                unique_tags.insert(tag.clone());
            }
        }
        let mut unique_tags: Vec<String> = unique_tags.into_iter().collect();
        unique_tags.sort();
        let total = unique_tags.len().max(1);
        unique_tags
            .into_iter()
            .enumerate()
            .map(|(i, tag)| (tag.clone(), tag_color(&tag, i, total)))
            .collect()
    };

    let graph = state.simulation.get_graph();

    let mut node_own_color: HashMap<NodeIndex, Color> = HashMap::new();
    let mut node_has_own_tags: HashMap<NodeIndex, bool> = HashMap::new();
    for idx in graph.node_indices() {
        let node = &graph[idx];
        if node.data.is_encrypted {
            node_own_color.insert(idx, Color::Red);
            node_has_own_tags.insert(idx, true);
        } else if let Some(tag) = node.data.tags.first() {
            node_own_color.insert(idx, tag_colors.get(tag).copied().unwrap_or(Color::Gray));
            node_has_own_tags.insert(idx, true);
        } else {
            node_own_color.insert(idx, Color::Gray);
            node_has_own_tags.insert(idx, false);
        }
    }

    let mut inherited_color: HashMap<NodeIndex, Color> = HashMap::new();
    for idx in graph.node_indices() {
        if node_has_own_tags.get(&idx).copied().unwrap_or(false) {
            continue;
        }
        let mut found_color: Option<Color> = None;
        for neighbor in graph.neighbors(idx) {
            if let Some(&color) = node_own_color.get(&neighbor) {
                if color != Color::Gray {
                    found_color = Some(color);
                    break;
                }
            }
        }
        inherited_color.insert(idx, found_color.unwrap_or(Color::Gray));
    }

    let final_node_color: HashMap<NodeIndex, Color> = graph
        .node_indices()
        .map(|idx| {
            if node_has_own_tags.get(&idx).copied().unwrap_or(false) {
                (idx, node_own_color[&idx])
            } else {
                (
                    idx,
                    inherited_color.get(&idx).copied().unwrap_or(Color::Gray),
                )
            }
        })
        .collect();

    let edges: Vec<EdgeData> = {
        use fdg_sim::petgraph::visit::IntoEdgeReferences;
        graph
            .edge_references()
            .map(|edge| {
                let src = &graph[edge.source()];
                let tgt = &graph[edge.target()];
                EdgeData {
                    x1: src.location.x as f64,
                    y1: src.location.y as f64,
                    x2: tgt.location.x as f64,
                    y2: tgt.location.y as f64,
                }
            })
            .collect()
    };

    let nodes: Vec<NodeData> = graph
        .node_indices()
        .map(|idx| {
            let node = &graph[idx];
            let primary_color = final_node_color.get(&idx).copied().unwrap_or(Color::Gray);
            let extra_tag_colors: Vec<Color> =
                if node.data.is_encrypted || node.data.tags.is_empty() {
                    Vec::new()
                } else {
                    node.data
                        .tags
                        .iter()
                        .skip(1)
                        .filter_map(|tag| tag_colors.get(tag).copied())
                        .collect()
                };
            NodeData {
                x: node.location.x as f64,
                y: node.location.y as f64,
                color: primary_color,
                extra_tag_colors,
                is_selected: state.selected_node == Some(idx),
            }
        })
        .collect();

    let labels: Vec<LabelData> = {
        let should_show = |idx: NodeIndex| -> bool {
            match label_mode {
                crate::config::GraphLabelMode::Selected => state.selected_node == Some(idx),
                crate::config::GraphLabelMode::Neighbors => {
                    if state.selected_node == Some(idx) {
                        return true;
                    }
                    if let Some(sel) = state.selected_node {
                        for edge in graph.edges(sel) {
                            if edge.target() == idx || edge.source() == idx {
                                return true;
                            }
                        }
                    }
                    false
                }
                crate::config::GraphLabelMode::All => true,
            }
        };

        graph
            .node_indices()
            .filter(|idx| should_show(*idx))
            .map(|idx| {
                let node = &graph[idx];
                LabelData {
                    x: node.location.x as f64,
                    y: node.location.y as f64 + 4.0,
                    text: truncate_owned(&node.data.title, 20),
                }
            })
            .collect()
    };

    let node_count = graph.node_count();
    let edge_count = graph.edge_count();

    let x_bounds = viewport.x_bounds(aspect);
    let y_bounds = viewport.y_bounds(aspect);

    let canvas = Canvas::default()
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title("Graph View"),
        )
        .marker(ratatui::symbols::Marker::Braille)
        .paint(move |ctx| {
            ctx.draw(&GraphEdgesData {
                edges: edges.clone(),
            });
            ctx.layer();
            ctx.draw(&GraphNodesData {
                nodes: nodes.clone(),
            });

            for label in &labels {
                let span = ratatui::text::Span::styled(
                    label.text.clone(),
                    ratatui::style::Style::default().fg(Color::Gray),
                );
                ctx.print(label.x, label.y, span);
            }
        });

    frame.render_widget(canvas, area);

    let selected_info = state
        .selected_node
        .and_then(|idx| graph.node_weight(idx))
        .map(|n| {
            let enc = if n.data.is_encrypted { " [ENC]" } else { "" };
            format!(" | Selected: {}{}", n.data.title, enc)
        })
        .unwrap_or_default();

    let status = format!(
        "Notes: {} | Links: {}{} | Esc: back | +/-: zoom | Scroll: zoom | Drag: move",
        node_count, edge_count, selected_info
    );

    let status_bar = ratatui::widgets::Paragraph::new(status)
        .style(ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray));
    let status_area = ratatui::layout::Rect::new(
        area.x + 1,
        area.y + area.height.saturating_sub(1),
        area.width.saturating_sub(2),
        1,
    );
    frame.render_widget(status_bar, status_area);
}

fn truncate_owned(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut end = max_len.saturating_sub(1);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}
