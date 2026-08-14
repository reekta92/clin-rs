use crate::draw::state::{DrawElement, DrawItem, DrawTransform, Shape, Viewport};
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

    #[must_use]
    pub fn contains_with_tolerance(self, point: (f64, f64), tolerance: f64) -> bool {
        point.0 >= self.min_x - tolerance
            && point.0 <= self.max_x + tolerance
            && point.1 >= self.min_y - tolerance
            && point.1 <= self.max_y + tolerance
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

    #[must_use]
    pub const fn scale(self) -> f64 {
        self.scale
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
            Shape::Line { x1, y1, x2, y2, .. } => DrawBounds::from_corners(*x1, *y1, *x2, *y2),
            Shape::Arrow { x1, y1, x2, y2, .. } => {
                let [left, right] = arrow_head_points(*x1, *y1, *x2, *y2);
                DrawBounds::from_points(&[(*x1, *y1), (*x2, *y2), left, right])
                    .expect("arrow has four bounds points")
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

pub fn transformed_bounds(item: &DrawItem) -> Option<DrawBounds> {
    transformed_bounds_with_affine(item, DrawAffine::new(&item.transform))
}

#[must_use]
pub fn transformed_bounds_with_transform(
    item: &DrawItem,
    transform: &DrawTransform,
) -> Option<DrawBounds> {
    transformed_bounds_with_affine(item, DrawAffine::new(transform))
}

#[must_use]
pub fn selection_handle_points(
    item: &DrawItem,
    transform: &DrawTransform,
    viewport: &Viewport,
) -> Option<((f64, f64), (f64, f64))> {
    let zoom = viewport.zoom.abs();
    if !zoom.is_finite() || zoom == 0.0 {
        return None;
    }
    let bounds = transformed_bounds_with_transform(item, transform)?;
    let center = bounds.center();
    let rotation = (center.0, bounds.min_y - 8.0 / zoom);
    let scale = (bounds.max_x, bounds.max_y);
    Some((rotation, scale))
}

fn transformed_bounds_with_affine(item: &DrawItem, transform: DrawAffine) -> Option<DrawBounds> {
    let bounds = base_bounds(&item.element)?;
    if matches!(item.element, DrawElement::Text(_)) {
        return Some(bounds.translated(item.transform.translate_x, item.transform.translate_y));
    }

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

#[must_use]
pub fn hit_test_item(
    item: &DrawItem,
    world_point: (f64, f64),
    screen_tolerance: f64,
    viewport: &Viewport,
) -> bool {
    let zoom = viewport.zoom.abs();
    if !zoom.is_finite()
        || zoom == 0.0
        || !screen_tolerance.is_finite()
        || !item.transform.scale.is_finite()
        || item.transform.scale <= 0.0
    {
        return false;
    }
    let world_tolerance = screen_tolerance.abs() / zoom;

    if matches!(item.element, DrawElement::Text(_)) {
        return transformed_bounds(item)
            .is_some_and(|bounds| bounds.contains_with_tolerance(world_point, world_tolerance));
    }

    let transform = DrawAffine::new(&item.transform);
    let Some(bounds) = transformed_bounds_with_affine(item, transform) else {
        return false;
    };
    if !bounds.contains_with_tolerance(world_point, world_tolerance) {
        return false;
    }

    let local_point = transform.inverse_transform_point(world_point);
    hit_test_element(
        &item.element,
        local_point,
        world_tolerance / transform.scale().abs(),
    )
}

fn hit_test_element(element: &DrawElement, point: (f64, f64), tolerance: f64) -> bool {
    match element {
        DrawElement::Stroke(stroke) => {
            stroke.points.windows(2).any(|window| {
                let [start, end] = window else {
                    return false;
                };
                point_to_segment_distance(point, *start, *end) <= tolerance
            }) || stroke
                .points
                .first()
                .is_some_and(|first| point_to_segment_distance(point, *first, *first) <= tolerance)
        }
        DrawElement::Shape(shape) => hit_test_shape(shape, point, tolerance),
        DrawElement::Text(_) => base_bounds(element)
            .is_some_and(|bounds| bounds.contains_with_tolerance(point, tolerance)),
    }
}

fn hit_test_shape(shape: &Shape, point: (f64, f64), tolerance: f64) -> bool {
    match shape {
        Shape::Rect {
            x,
            y,
            width,
            height,
            ..
        } => {
            let bounds = DrawBounds::from_corners(*x, *y, *x + *width, *y + *height);
            let corners = bounds.corners();
            closed_segments_hit(&corners, point, tolerance)
        }
        Shape::Ellipse {
            x,
            y,
            width,
            height,
            ..
        } => {
            let bounds = DrawBounds::from_corners(*x, *y, *x + *width, *y + *height);
            closest_ellipse_distance(point, bounds) <= tolerance
        }
        Shape::Diamond {
            x,
            y,
            width,
            height,
            ..
        } => {
            let bounds = DrawBounds::from_corners(*x, *y, *x + *width, *y + *height);
            let points = [
                ((bounds.min_x + bounds.max_x) / 2.0, bounds.min_y),
                (bounds.max_x, (bounds.min_y + bounds.max_y) / 2.0),
                ((bounds.min_x + bounds.max_x) / 2.0, bounds.max_y),
                (bounds.min_x, (bounds.min_y + bounds.max_y) / 2.0),
            ];
            closed_segments_hit(&points, point, tolerance)
        }
        Shape::Line { x1, y1, x2, y2, .. } => {
            point_to_segment_distance(point, (*x1, *y1), (*x2, *y2)) <= tolerance
        }
        Shape::Arrow { x1, y1, x2, y2, .. } => {
            let start = (*x1, *y1);
            let end = (*x2, *y2);
            let [left, right] = arrow_head_points(*x1, *y1, *x2, *y2);
            point_to_segment_distance(point, start, end) <= tolerance
                || point_to_segment_distance(point, end, left) <= tolerance
                || point_to_segment_distance(point, end, right) <= tolerance
        }
    }
}

fn closed_segments_hit(points: &[(f64, f64); 4], point: (f64, f64), tolerance: f64) -> bool {
    point_to_segment_distance(point, points[0], points[1]) <= tolerance
        || point_to_segment_distance(point, points[1], points[2]) <= tolerance
        || point_to_segment_distance(point, points[2], points[3]) <= tolerance
        || point_to_segment_distance(point, points[3], points[0]) <= tolerance
}

fn point_to_segment_distance(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let delta_x = end.0 - start.0;
    let delta_y = end.1 - start.1;
    let length_squared = delta_x.mul_add(delta_x, delta_y * delta_y);
    if length_squared == 0.0 {
        return (point.0 - start.0).hypot(point.1 - start.1);
    }
    let projection =
        ((point.0 - start.0) * delta_x + (point.1 - start.1) * delta_y) / length_squared;
    let projection = projection.clamp(0.0, 1.0);
    (point.0 - (start.0 + projection * delta_x)).hypot(point.1 - (start.1 + projection * delta_y))
}

fn arrow_head_points(x1: f64, y1: f64, x2: f64, y2: f64) -> [(f64, f64); 2] {
    let angle = (y2 - y1).atan2(x2 - x1);
    let head_length = 5.0;
    let head_angle = std::f64::consts::PI / 6.0;
    [
        (
            x2 - head_length * (angle - head_angle).cos(),
            y2 - head_length * (angle - head_angle).sin(),
        ),
        (
            x2 - head_length * (angle + head_angle).cos(),
            y2 - head_length * (angle + head_angle).sin(),
        ),
    ]
}

fn closest_ellipse_distance(point: (f64, f64), bounds: DrawBounds) -> f64 {
    let radius_x = (bounds.max_x - bounds.min_x) / 2.0;
    let radius_y = (bounds.max_y - bounds.min_y) / 2.0;
    let center = bounds.center();
    if radius_x == 0.0 && radius_y == 0.0 {
        return point_to_segment_distance(point, center, center);
    }
    if radius_x == 0.0 {
        return point_to_segment_distance(
            point,
            (center.0, center.1 - radius_y),
            (center.0, center.1 + radius_y),
        );
    }
    if radius_y == 0.0 {
        return point_to_segment_distance(
            point,
            (center.0 - radius_x, center.1),
            (center.0 + radius_x, center.1),
        );
    }

    let delta_x = point.0 - center.0;
    let delta_y = point.1 - center.1;
    if delta_x == 0.0 && delta_y == 0.0 {
        return radius_x.min(radius_y);
    }

    let initial = (delta_y / radius_y).atan2(delta_x / radius_x);
    [
        initial,
        initial + std::f64::consts::FRAC_PI_2,
        initial + std::f64::consts::PI,
        initial + 3.0 * std::f64::consts::FRAC_PI_2,
    ]
    .into_iter()
    .map(|angle| ellipse_distance_from_angle(point, center, radius_x, radius_y, angle))
    .fold(f64::INFINITY, f64::min)
}

fn ellipse_distance_from_angle(
    point: (f64, f64),
    center: (f64, f64),
    radius_x: f64,
    radius_y: f64,
    mut angle: f64,
) -> f64 {
    for _ in 0..8 {
        let sin = angle.sin();
        let cos = angle.cos();
        let edge_x = center.0 + radius_x * cos;
        let edge_y = center.1 + radius_y * sin;
        let delta_x = edge_x - point.0;
        let delta_y = edge_y - point.1;
        let first = delta_x * -radius_x * sin + delta_y * radius_y * cos;
        let second = (radius_x * sin).powi(2) + (radius_y * cos).powi(2)
            - delta_x * radius_x * cos
            - delta_y * radius_y * sin;
        if second.abs() < f64::EPSILON {
            break;
        }
        let step = (first / second).clamp(-0.5, 0.5);
        angle -= step;
        if step.abs() < 1e-9 {
            break;
        }
    }
    let edge = (
        center.0 + radius_x * angle.cos(),
        center.1 + radius_y * angle.sin(),
    );
    (point.0 - edge.0).hypot(point.1 - edge.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::state::{Stroke, Text};

    fn item(element: DrawElement) -> DrawItem {
        DrawItem::new(element)
    }

    fn assert_close(actual: (f64, f64), expected: (f64, f64)) {
        assert!(
            (actual.0 - expected.0).abs() < 1e-9,
            "x: {actual:?} != {expected:?}"
        );
        assert!(
            (actual.1 - expected.1).abs() < 1e-9,
            "y: {actual:?} != {expected:?}"
        );
    }

    #[test]
    fn affine_round_trip_and_rotations_preserve_points() {
        for rotation_degrees in [0.0, 90.0, 359.0] {
            let transform = DrawTransform {
                pivot_x: 10.0,
                pivot_y: -3.0,
                translate_x: 7.0,
                translate_y: 11.0,
                rotation_degrees,
                scale: 1.75,
            };
            let world = transform_point(&transform, (16.0, 9.0));
            assert_close(inverse_transform_point(&transform, world), (16.0, 9.0));
        }
    }

    #[test]
    fn bounds_normalize_negative_sizes_and_translate_text_only() {
        let rectangle = DrawElement::Shape(Shape::Rect {
            x: 10.0,
            y: 20.0,
            width: -6.0,
            height: -8.0,
            color: (0, 0, 0),
        });
        assert_eq!(
            base_bounds(&rectangle),
            Some(DrawBounds::from_corners(4.0, 12.0, 10.0, 20.0))
        );

        let mut text = item(DrawElement::Text(Text {
            content: "wide界".to_string(),
            x: 2.0,
            y: 3.0,
            color: (0, 0, 0),
        }));
        text.transform.translate_x = 10.0;
        text.transform.translate_y = -4.0;
        assert_eq!(translated_text_position(&text), Some((12.0, -1.0)));
        assert_eq!(
            transformed_bounds(&text),
            Some(DrawBounds::from_corners(
                12.0,
                -1.0,
                12.0 + "wide界".width() as f64 * TEXT_CHAR_WIDTH,
                -1.0 + TEXT_HEIGHT,
            ))
        );
    }

    #[test]
    fn hit_tests_precisely_cover_each_element_kind() {
        let viewport = Viewport::default();
        let tolerance = 0.5;

        let stroke = item(DrawElement::Stroke(Stroke {
            points: vec![(0.0, 0.0), (10.0, 0.0)],
            color: (0, 0, 0),
        }));
        assert!(hit_test_item(&stroke, (5.0, 0.25), tolerance, &viewport));
        assert!(!hit_test_item(&stroke, (5.0, 2.0), tolerance, &viewport));

        let rectangle = item(DrawElement::Shape(Shape::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            color: (0, 0, 0),
        }));
        assert!(hit_test_item(&rectangle, (0.0, 5.0), tolerance, &viewport));
        assert!(!hit_test_item(&rectangle, (5.0, 5.0), tolerance, &viewport));

        let ellipse = item(DrawElement::Shape(Shape::Ellipse {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 6.0,
            color: (0, 0, 0),
        }));
        assert!(hit_test_item(&ellipse, (10.0, 3.0), tolerance, &viewport));
        assert!(!hit_test_item(&ellipse, (5.0, 3.0), tolerance, &viewport));

        let diamond = item(DrawElement::Shape(Shape::Diamond {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            color: (0, 0, 0),
        }));
        assert!(hit_test_item(&diamond, (5.0, 0.0), tolerance, &viewport));
        assert!(!hit_test_item(&diamond, (5.0, 5.0), tolerance, &viewport));

        let line = item(DrawElement::Shape(Shape::Line {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
            color: (0, 0, 0),
        }));
        assert!(hit_test_item(&line, (5.0, 5.25), tolerance, &viewport));

        let arrow = item(DrawElement::Shape(Shape::Arrow {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 0.0,
            color: (0, 0, 0),
        }));
        assert!(hit_test_item(&arrow, (10.0, 0.0), tolerance, &viewport));
        assert!(hit_test_item(&arrow, (6.0, 2.0), tolerance, &viewport));

        let text = item(DrawElement::Text(Text {
            content: "text".to_string(),
            x: 0.0,
            y: 0.0,
            color: (0, 0, 0),
        }));
        assert!(hit_test_item(&text, (8.0, 6.0), tolerance, &viewport));
    }

    #[test]
    fn transformed_hit_tests_use_world_tolerance() {
        let mut rectangle = item(DrawElement::Shape(Shape::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            color: (0, 0, 0),
        }));
        rectangle.transform = DrawTransform {
            pivot_x: 5.0,
            pivot_y: 5.0,
            translate_x: 20.0,
            translate_y: 0.0,
            rotation_degrees: 90.0,
            scale: 2.0,
        };
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 2.0,
        };
        assert!(hit_test_item(&rectangle, (25.0, 15.0), 1.0, &viewport));
        assert!(!hit_test_item(&rectangle, (25.0, 13.0), 1.0, &viewport));
    }

    #[test]
    fn selection_handles_follow_transformed_bounds() {
        let mut rectangle = item(DrawElement::Shape(Shape::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 20.0,
            color: (0, 0, 0),
        }));
        rectangle.transform = DrawTransform {
            pivot_x: 5.0,
            pivot_y: 10.0,
            translate_x: 20.0,
            translate_y: -5.0,
            rotation_degrees: 0.0,
            scale: 2.0,
        };
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 2.0,
        };

        assert_eq!(
            selection_handle_points(&rectangle, &rectangle.transform, &viewport),
            Some(((25.0, -19.0), (35.0, 25.0)))
        );
    }
}
