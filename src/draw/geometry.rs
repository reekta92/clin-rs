use crate::draw::state::{DrawElement, DrawItem, DrawTransform, Shape};
use unicode_width::UnicodeWidthStr;

pub const TEXT_CHAR_WIDTH: f64 = 8.0;
pub const TEXT_HEIGHT: f64 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawBounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl DrawBounds {
    #[must_use]
    pub fn from_corners(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            min_x: x1.min(x2),
            min_y: y1.min(y2),
            max_x: x1.max(x2),
            max_y: y1.max(y2),
        }
    }

    #[must_use]
    pub fn from_points(points: &[(f64, f64)]) -> Option<Self> {
        let &(first_x, first_y) = points.first()?;
        let mut bounds = Self {
            min_x: first_x,
            min_y: first_y,
            max_x: first_x,
            max_y: first_y,
        };
        for &(x, y) in &points[1..] {
            bounds.min_x = bounds.min_x.min(x);
            bounds.min_y = bounds.min_y.min(y);
            bounds.max_x = bounds.max_x.max(x);
            bounds.max_y = bounds.max_y.max(y);
        }
        Some(bounds)
    }

    #[must_use]
    pub const fn center(self) -> (f64, f64) {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }

    #[must_use]
    pub const fn corners(self) -> [(f64, f64); 4] {
        [
            (self.min_x, self.min_y),
            (self.max_x, self.min_y),
            (self.max_x, self.max_y),
            (self.min_x, self.max_y),
        ]
    }

    #[must_use]
    pub const fn translated(self, x: f64, y: f64) -> Self {
        Self {
            min_x: self.min_x + x,
            min_y: self.min_y + y,
            max_x: self.max_x + x,
            max_y: self.max_y + y,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DrawAffine {
    pivot_x: f64,
    pivot_y: f64,
    translate_x: f64,
    translate_y: f64,
    scale: f64,
    sin: f64,
    cos: f64,
    identity: bool,
}

impl DrawAffine {
    #[must_use]
    pub fn new(transform: &DrawTransform) -> Self {
        let radians = transform.rotation_degrees.to_radians();
        Self {
            pivot_x: transform.pivot_x,
            pivot_y: transform.pivot_y,
            translate_x: transform.translate_x,
            translate_y: transform.translate_y,
            scale: transform.scale,
            sin: radians.sin(),
            cos: radians.cos(),
            identity: transform.translate_x == 0.0
                && transform.translate_y == 0.0
                && transform.rotation_degrees == 0.0
                && transform.scale == 1.0,
        }
    }

    #[must_use]
    pub const fn identity() -> Self {
        Self {
            pivot_x: 0.0,
            pivot_y: 0.0,
            translate_x: 0.0,
            translate_y: 0.0,
            scale: 1.0,
            sin: 0.0,
            cos: 1.0,
            identity: true,
        }
    }

    #[must_use]
    pub fn transform_point(self, point: (f64, f64)) -> (f64, f64) {
        let x = (point.0 - self.pivot_x) * self.scale;
        let y = (point.1 - self.pivot_y) * self.scale;
        (
            self.pivot_x + self.translate_x + x * self.cos - y * self.sin,
            self.pivot_y + self.translate_y + x * self.sin + y * self.cos,
        )
    }

    #[must_use]
    pub fn inverse_transform_point(self, point: (f64, f64)) -> (f64, f64) {
        let x = point.0 - self.pivot_x - self.translate_x;
        let y = point.1 - self.pivot_y - self.translate_y;
        (
            self.pivot_x + (x * self.cos + y * self.sin) / self.scale,
            self.pivot_y + (-x * self.sin + y * self.cos) / self.scale,
        )
    }

    #[must_use]
    pub const fn is_identity(self) -> bool {
        self.identity
    }
}

#[must_use]
pub fn base_bounds(element: &DrawElement) -> Option<DrawBounds> {
    match element {
        DrawElement::Stroke(stroke) => DrawBounds::from_points(&stroke.points),
        DrawElement::Shape(shape) => Some(match shape {
            Shape::Rect {
                x,
                y,
                width,
                height,
                ..
            }
            | Shape::Ellipse {
                x,
                y,
                width,
                height,
                ..
            }
            | Shape::Diamond {
                x,
                y,
                width,
                height,
                ..
            } => DrawBounds::from_corners(*x, *y, *x + *width, *y + *height),
            Shape::Line { x1, y1, x2, y2, .. } | Shape::Arrow { x1, y1, x2, y2, .. } => {
                DrawBounds::from_corners(*x1, *y1, *x2, *y2)
            }
        }),
        DrawElement::Text(text) => Some(DrawBounds::from_corners(
            text.x,
            text.y,
            text.x + text.content.width() as f64 * TEXT_CHAR_WIDTH,
            text.y + TEXT_HEIGHT,
        )),
    }
}

#[must_use]
pub fn transformed_bounds(item: &DrawItem) -> Option<DrawBounds> {
    let bounds = base_bounds(&item.element)?;
    if matches!(item.element, DrawElement::Text(_)) {
        return Some(bounds.translated(item.transform.translate_x, item.transform.translate_y));
    }

    let transform = DrawAffine::new(&item.transform);
    let corners = bounds.corners();
    let mut transformed = DrawBounds::from_points(&[transform.transform_point(corners[0])])?;
    for corner in &corners[1..] {
        let (x, y) = transform.transform_point(*corner);
        transformed.min_x = transformed.min_x.min(x);
        transformed.min_y = transformed.min_y.min(y);
        transformed.max_x = transformed.max_x.max(x);
        transformed.max_y = transformed.max_y.max(y);
    }
    Some(transformed)
}

#[must_use]
pub fn transform_point(transform: &DrawTransform, point: (f64, f64)) -> (f64, f64) {
    DrawAffine::new(transform).transform_point(point)
}

#[must_use]
pub fn inverse_transform_point(transform: &DrawTransform, point: (f64, f64)) -> (f64, f64) {
    DrawAffine::new(transform).inverse_transform_point(point)
}

#[must_use]
pub fn transform_item_point(item: &DrawItem, point: (f64, f64)) -> (f64, f64) {
    if matches!(item.element, DrawElement::Text(_)) {
        (
            point.0 + item.transform.translate_x,
            point.1 + item.transform.translate_y,
        )
    } else {
        DrawAffine::new(&item.transform).transform_point(point)
    }
}

#[must_use]
pub fn inverse_transform_item_point(item: &DrawItem, point: (f64, f64)) -> (f64, f64) {
    if matches!(item.element, DrawElement::Text(_)) {
        (
            point.0 - item.transform.translate_x,
            point.1 - item.transform.translate_y,
        )
    } else {
        DrawAffine::new(&item.transform).inverse_transform_point(point)
    }
}

#[must_use]
pub fn translated_text_position(item: &DrawItem) -> Option<(f64, f64)> {
    let DrawElement::Text(text) = &item.element else {
        return None;
    };
    Some((
        text.x + item.transform.translate_x,
        text.y + item.transform.translate_y,
    ))
}
