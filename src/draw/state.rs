use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use uuid::Uuid;

pub const DRAW_SCHEMA_VERSION: u8 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct DrawData {
    pub version: u8,
    pub width: f64,
    pub height: f64,
    pub background: Option<String>,
    pub elements: Vec<DrawItem>,
}

impl Default for DrawData {
    fn default() -> Self {
        Self {
            version: DRAW_SCHEMA_VERSION,
            width: 1000.0,
            height: 1000.0,
            background: None,
            elements: Vec::new(),
        }
    }
}

impl DrawData {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != DRAW_SCHEMA_VERSION {
            return Err(format!(
                "draw schema version {} cannot be serialized as version {DRAW_SCHEMA_VERSION}",
                self.version
            ));
        }

        let mut ids = HashSet::with_capacity(self.elements.len());
        for item in &self.elements {
            item.id.validate()?;
            if !ids.insert(item.id.as_str()) {
                return Err(format!("draw item ID '{}' is duplicated", item.id));
            }
            item.transform.validate_for(&item.element)?;
        }
        Ok(())
    }

    #[must_use]
    pub fn item(&self, id: &DrawItemId) -> Option<&DrawItem> {
        self.elements.iter().find(|item| &item.id == id)
    }

    pub fn item_mut(&mut self, id: &DrawItemId) -> Option<&mut DrawItem> {
        self.elements.iter_mut().find(|item| &item.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DrawItemId(pub String);

impl DrawItemId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<(), String> {
        if self.0.is_empty() {
            return Err("draw item ID cannot be empty".to_string());
        }
        let uuid = Uuid::parse_str(&self.0)
            .map_err(|_| format!("draw item ID '{}' is not a UUID", self.0))?;
        if uuid.get_version_num() != 4 {
            return Err(format!("draw item ID '{}' is not a UUID v4", self.0));
        }
        Ok(())
    }
}

impl std::fmt::Display for DrawItemId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawItem {
    pub id: DrawItemId,
    pub element: DrawElement,
    pub transform: DrawTransform,
}

impl DrawItem {
    #[must_use]
    pub fn new(element: DrawElement) -> Self {
        let (pivot_x, pivot_y) = crate::draw::geometry::base_bounds(&element)
            .map_or((0.0, 0.0), |bounds| bounds.center());
        Self {
            id: DrawItemId::new(),
            transform: DrawTransform::identity(pivot_x, pivot_y),
            element,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawClipboard {
    pub item: DrawItem,
}

impl DrawClipboard {
    #[must_use]
    pub fn from_item(item: &DrawItem) -> Self {
        Self { item: item.clone() }
    }

    #[must_use]
    pub fn pasted_item(&self) -> DrawItem {
        DrawItem {
            id: DrawItemId::new(),
            element: self.item.element.clone(),
            transform: self.item.transform,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DrawTransform {
    pub pivot_x: f64,
    pub pivot_y: f64,
    pub translate_x: f64,
    pub translate_y: f64,
    pub rotation_degrees: f64,
    pub scale: f64,
}

impl DrawTransform {
    #[must_use]
    pub const fn identity(pivot_x: f64, pivot_y: f64) -> Self {
        Self {
            pivot_x,
            pivot_y,
            translate_x: 0.0,
            translate_y: 0.0,
            rotation_degrees: 0.0,
            scale: 1.0,
        }
    }

    fn validate_for(&self, element: &DrawElement) -> Result<(), String> {
        let fields = [
            ("pivot_x", self.pivot_x),
            ("pivot_y", self.pivot_y),
            ("translate_x", self.translate_x),
            ("translate_y", self.translate_y),
            ("rotation_degrees", self.rotation_degrees),
            ("scale", self.scale),
        ];
        for (name, value) in fields {
            if !value.is_finite() {
                return Err(format!("draw transform {name} must be finite"));
            }
        }
        if self.scale <= 0.0 {
            return Err("draw transform scale must be positive".to_string());
        }
        if matches!(element, DrawElement::Text(_))
            && (self.rotation_degrees != 0.0 || self.scale != 1.0)
        {
            return Err("text draw items require identity rotation and scale".to_string());
        }
        Ok(())
    }
}

impl Default for DrawTransform {
    fn default() -> Self {
        Self::identity(0.0, 0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct V2DrawData {
    version: u8,
    #[serde(default = "default_draw_dimension")]
    width: f64,
    #[serde(default = "default_draw_dimension")]
    height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background: Option<String>,
    elements: Vec<DrawItem>,
}

#[derive(Deserialize)]
struct LegacyDrawData {
    #[serde(default)]
    version: Option<u8>,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    elements: Vec<LegacyDrawElement>,
}

#[derive(Deserialize)]
enum LegacyDrawElement {
    Stroke(Stroke),
    Shape(Shape),
    Text(Text),
    Image(serde::de::IgnoredAny),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum DrawDataWire {
    V2(V2DrawData),
    Legacy(LegacyDrawData),
}

impl Serialize for DrawData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(S::Error::custom)?;
        V2DrawData {
            version: DRAW_SCHEMA_VERSION,
            width: self.width,
            height: self.height,
            background: self.background.clone(),
            elements: self.elements.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DrawData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match DrawDataWire::deserialize(deserializer)? {
            DrawDataWire::V2(data) => {
                if data.version > DRAW_SCHEMA_VERSION {
                    return Err(D::Error::custom(format!(
                        "unsupported draw schema version {}; newest supported version is {DRAW_SCHEMA_VERSION}",
                        data.version
                    )));
                }
                if data.version != DRAW_SCHEMA_VERSION {
                    return Err(D::Error::custom(format!(
                        "draw schema version {} uses v2 item records",
                        data.version
                    )));
                }
                let data = Self {
                    version: DRAW_SCHEMA_VERSION,
                    width: data.width,
                    height: data.height,
                    background: data.background,
                    elements: data.elements,
                };
                data.validate().map_err(D::Error::custom)?;
                Ok(data)
            }
            DrawDataWire::Legacy(data) => {
                let legacy_version = data.version.unwrap_or(0);
                if legacy_version > DRAW_SCHEMA_VERSION {
                    return Err(D::Error::custom(format!(
                        "unsupported draw schema version {legacy_version}; newest supported version is {DRAW_SCHEMA_VERSION}"
                    )));
                }
                if legacy_version == DRAW_SCHEMA_VERSION {
                    return Err(D::Error::custom(
                        "draw schema version 2 requires item IDs and transforms",
                    ));
                }

                let elements = data
                    .elements
                    .into_iter()
                    .filter_map(|element| match element {
                        LegacyDrawElement::Stroke(stroke) => {
                            Some(DrawItem::new(DrawElement::Stroke(stroke)))
                        }
                        LegacyDrawElement::Shape(shape) => {
                            Some(DrawItem::new(DrawElement::Shape(shape)))
                        }
                        LegacyDrawElement::Text(text) => {
                            Some(DrawItem::new(DrawElement::Text(text)))
                        }
                        LegacyDrawElement::Image(_) => None,
                    })
                    .collect();
                let data = Self {
                    version: DRAW_SCHEMA_VERSION,
                    width: data.width.unwrap_or_else(default_draw_dimension),
                    height: data.height.unwrap_or_else(default_draw_dimension),
                    background: data.background,
                    elements,
                };
                data.validate().map_err(D::Error::custom)?;
                Ok(data)
            }
        }
    }
}

const fn default_draw_dimension() -> f64 {
    1000.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DrawElement {
    Stroke(Stroke),
    Shape(Shape),
    Text(Text),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub points: Vec<(f64, f64)>,
    pub color: (u8, u8, u8),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub content: String,
    pub x: f64,
    pub y: f64,
    pub color: (u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawTool {
    Cursor,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke() -> DrawElement {
        DrawElement::Stroke(Stroke {
            points: vec![(10.0, 20.0), (30.0, 40.0)],
            color: (1, 2, 3),
        })
    }

    fn v2_item_value(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "element": {"Stroke": {"points": [[0.0, 0.0], [10.0, 10.0]], "color": [1, 2, 3]}},
            "transform": {
                "pivot_x": 5.0,
                "pivot_y": 5.0,
                "translate_x": 1.0,
                "translate_y": 2.0,
                "rotation_degrees": 30.0,
                "scale": 2.0
            }
        })
    }

    #[test]
    fn v2_round_trip_preserves_items_and_transforms() {
        let mut item = DrawItem::new(stroke());
        item.transform = DrawTransform {
            pivot_x: 20.0,
            pivot_y: 30.0,
            translate_x: 5.0,
            translate_y: -8.0,
            rotation_degrees: 90.0,
            scale: 1.5,
        };
        let data = DrawData {
            version: DRAW_SCHEMA_VERSION,
            width: 200.0,
            height: 300.0,
            background: Some("#010203".to_string()),
            elements: vec![item],
        };

        let encoded = serde_json::to_string(&data).unwrap();
        assert!(encoded.contains("\"version\":2"));
        assert_eq!(serde_json::from_str::<DrawData>(&encoded).unwrap(), data);
    }

    #[test]
    fn legacy_versions_migrate_and_drop_images() {
        for version in [None, Some(0), Some(1)] {
            let mut source = serde_json::json!({
                "width": 200.0,
                "height": 300.0,
                "elements": [
                    {"Stroke": {"points": [[1.0, 2.0], [3.0, 4.0]], "color": [1, 2, 3]}},
                    {"Image": {"id": "legacy", "path": "image.png", "x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0}},
                    {"Text": {"content": "legacy", "x": 20.0, "y": 30.0, "color": [4, 5, 6]}}
                ]
            });
            if let Some(version) = version {
                source["version"] = serde_json::json!(version);
            }

            let data: DrawData = serde_json::from_value(source).unwrap();
            assert_eq!(data.version, DRAW_SCHEMA_VERSION);
            assert_eq!(data.elements.len(), 2);
            assert!(data.elements.iter().all(|item| {
                Uuid::parse_str(item.id.as_str()).is_ok()
                    && item.transform.scale == 1.0
                    && item.transform.rotation_degrees == 0.0
                    && item.transform.translate_x == 0.0
                    && item.transform.translate_y == 0.0
            }));
        }
    }

    #[test]
    fn v2_rejects_future_duplicate_and_invalid_transforms() {
        let id = Uuid::new_v4().to_string();
        let future = serde_json::json!({
            "version": 3,
            "width": 100.0,
            "height": 100.0,
            "elements": [v2_item_value(&id)]
        });
        assert!(
            serde_json::from_value::<DrawData>(future)
                .unwrap_err()
                .to_string()
                .contains("unsupported draw schema version 3")
        );

        let duplicate = serde_json::json!({
            "version": 2,
            "width": 100.0,
            "height": 100.0,
            "elements": [v2_item_value(&id), v2_item_value(&id)]
        });
        assert!(
            serde_json::from_value::<DrawData>(duplicate)
                .unwrap_err()
                .to_string()
                .contains("duplicated")
        );

        let mut data = DrawData::default();
        data.elements.push(DrawItem {
            id: DrawItemId::new(),
            element: DrawElement::Text(Text {
                content: "text".to_string(),
                x: 0.0,
                y: 0.0,
                color: (255, 255, 255),
            }),
            transform: DrawTransform {
                scale: 2.0,
                ..DrawTransform::default()
            },
        });
        assert!(
            data.validate()
                .unwrap_err()
                .contains("identity rotation and scale")
        );

        data.elements[0].element = stroke();
        data.elements[0].transform.scale = 0.0;
        assert!(
            data.validate()
                .unwrap_err()
                .contains("scale must be positive")
        );
    }

    #[test]
    fn clipboard_preserves_item_payload_and_refreshes_id() {
        let mut source = DrawItem::new(stroke());
        source.transform.translate_x = 12.0;
        source.transform.rotation_degrees = 45.0;
        source.transform.scale = 2.0;

        let pasted = DrawClipboard::from_item(&source).pasted_item();
        assert_ne!(pasted.id, source.id);
        assert_eq!(pasted.element, source.element);
        assert_eq!(pasted.transform, source.transform);
    }
}
