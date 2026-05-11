use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone)]
pub struct DrawData {
    pub version: u8,
    pub width: f64,
    pub height: f64,
    pub background: Option<String>,
    pub elements: Vec<DrawElement>,
}

impl Default for DrawData {
    fn default() -> Self {
        Self {
            version: 1,
            width: 1000.0,
            height: 1000.0,
            background: None,
            elements: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NewDrawData {
    pub version: u8,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    pub elements: Vec<DrawElement>,
}

impl Serialize for DrawData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let new = NewDrawData {
            version: self.version,
            width: self.width,
            height: self.height,
            background: self.background.clone(),
            elements: self.elements.clone(),
        };
        new.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DrawData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DrawDataVisitor;

        impl<'de> Visitor<'de> for DrawDataVisitor {
            type Value = DrawData;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a DrawData struct or an old-format CanvasData struct")
            }

            fn visit_map<M>(self, mut map: M) -> Result<DrawData, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut version: Option<u8> = None;
                let mut width: Option<f64> = None;
                let mut height: Option<f64> = None;
                let mut background: Option<Option<String>> = None;
                let mut elements: Option<Vec<DrawElement>> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "version" => version = Some(map.next_value()?),
                        "width" => width = Some(map.next_value()?),
                        "height" => height = Some(map.next_value()?),
                        "background" => background = Some(map.next_value()?),
                        "elements" => elements = Some(map.next_value()?),
                        _ => {
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let elements = elements.unwrap_or_default();

                if version.is_none() {
                    return Ok(DrawData {
                        version: 0,
                        width: 1000.0,
                        height: 1000.0,
                        background: None,
                        elements,
                    });
                }

                Ok(DrawData {
                    version: version.unwrap_or(1),
                    width: width.unwrap_or(1000.0),
                    height: height.unwrap_or(1000.0),
                    background: background.unwrap_or_default(),
                    elements,
                })
            }
        }

        deserializer.deserialize_map(DrawDataVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DrawElement {
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
pub enum DrawShapeType {
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
pub enum DrawTool {
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
