use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasData {
    pub elements: Vec<CanvasElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanvasElement {
    Stroke(Stroke),
    Shape(Shape),
    Text(Text),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stroke {
    pub points: Vec<(f64, f64)>,
    pub color: (u8, u8, u8),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Shape {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: (u8, u8, u8),
    },
    Ellipse {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: (u8, u8, u8),
    },
    Diamond {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: (u8, u8, u8),
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: (u8, u8, u8),
    },
    Arrow {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: (u8, u8, u8),
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeType {
    Rect,
    Ellipse,
    Diamond,
    Line,
    Arrow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Text {
    pub content: String,
    pub x: f64,
    pub y: f64,
    pub color: (u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasTool {
    Draw,
    Erase,
    Text,
    Shape,
}

#[derive(Debug, Clone)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

impl Default for CanvasData {
    fn default() -> Self {
        Self {
            elements: Vec::new(),
        }
    }
}
