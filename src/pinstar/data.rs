use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CanvasData {
    pub nodes: Vec<CanvasNode>,
    pub edges: Vec<CanvasEdge>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CanvasNode {
    Text(TextNode),
    File(FileNode),
    Link(LinkNode),
    Group(GroupNode),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text: String,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub file: String,
    pub subpath: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LinkNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub url: String,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GroupNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub label: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CanvasEdge {
    pub id: String,
    pub from_node: String,
    pub from_side: Option<String>,
    pub to_node: String,
    pub to_side: Option<String>,
    pub label: Option<String>,
    pub color: Option<String>,
}

impl CanvasNode {
    pub fn id(&self) -> &str {
        match self {
            CanvasNode::Text(n) => &n.id,
            CanvasNode::File(n) => &n.id,
            CanvasNode::Link(n) => &n.id,
            CanvasNode::Group(n) => &n.id,
        }
    }

    pub fn pos(&self) -> (f64, f64) {
        match self {
            CanvasNode::Text(n) => (n.x, n.y),
            CanvasNode::File(n) => (n.x, n.y),
            CanvasNode::Link(n) => (n.x, n.y),
            CanvasNode::Group(n) => (n.x, n.y),
        }
    }

    pub fn size(&self) -> (f64, f64) {
        match self {
            CanvasNode::Text(n) => (n.width, n.height),
            CanvasNode::File(n) => (n.width, n.height),
            CanvasNode::Link(n) => (n.width, n.height),
            CanvasNode::Group(n) => (n.width, n.height),
        }
    }

    pub fn text(&self) -> &str {
        match self {
            CanvasNode::Text(n) => &n.text,
            CanvasNode::File(n) => &n.file,
            CanvasNode::Link(n) => &n.url,
            CanvasNode::Group(n) => n.label.as_deref().unwrap_or(""),
        }
    }

    pub fn set_text(&mut self, text: String) {
        match self {
            CanvasNode::Text(n) => n.text = text,
            CanvasNode::File(n) => n.file = text,
            CanvasNode::Link(n) => n.url = text,
            CanvasNode::Group(n) => n.label = Some(text),
        }
    }
}
