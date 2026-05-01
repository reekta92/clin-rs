use std::collections::HashMap;

use fdg_sim::petgraph::visit::EdgeRef;
use ratatui::style::Color;
use ratatui::widgets::canvas::{Canvas, Line, Painter, Shape};

use super::GraphState;

const TAG_PALETTE: &[Color] = &[
    Color::Cyan,
    Color::Yellow,
    Color::Green,
    Color::Magenta,
    Color::Blue,
    Color::LightRed,
    Color::LightCyan,
    Color::LightGreen,
    Color::LightMagenta,
    Color::LightBlue,
];

fn tag_color(tag: &str) -> Color {
    let hash = tag
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    TAG_PALETTE[(hash as usize) % TAG_PALETTE.len()]
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
        }
    }
}

pub fn draw_graph_view(frame: &mut ratatui::Frame, state: &GraphState) {
    let area = frame.area();
    let aspect = area.width as f64 / area.height as f64;
    let viewport = &state.viewport;

    let tag_colors: HashMap<String, Color> = {
        let graph = state.simulation.get_graph();
        let mut colors = HashMap::new();
        for node in graph.node_weights() {
            if let Some(tag) = node.data.tags.first() {
                colors.entry(tag.clone()).or_insert_with(|| tag_color(tag));
            }
        }
        colors
    };

    let graph = state.simulation.get_graph();

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
            let color = if node.data.is_encrypted {
                Color::Red
            } else if let Some(tag) = node.data.tags.first() {
                tag_colors.get(tag).copied().unwrap_or(Color::Gray)
            } else {
                Color::Gray
            };
            NodeData {
                x: node.location.x as f64,
                y: node.location.y as f64,
                color,
                is_selected: state.selected_node == Some(idx),
            }
        })
        .collect();

    let labels: Vec<LabelData> = if state.show_labels {
        graph
            .node_indices()
            .map(|idx| {
                let node = &graph[idx];
                LabelData {
                    x: node.location.x as f64,
                    y: node.location.y as f64 + 4.0,
                    text: truncate_owned(&node.data.title, 20),
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let node_count = graph.node_count();
    let edge_count = graph.edge_count();

    let x_bounds = viewport.x_bounds(aspect);
    let y_bounds = viewport.y_bounds();

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
                ctx.print(label.x, label.y, label.text.clone());
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
