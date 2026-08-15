use crate::app::ViewMode;
use crate::draw::app::{DrawAppState, DrawInteraction};
use crate::draw::state::{
    DrawElement, DrawItem, DrawShapeType, DrawTool, DrawTransform, Shape, Stroke,
};
use crate::keybinds::DrawAction;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::canvas::{Canvas, Context, Line, Rectangle};

/// Draw-view tool tab (label, glyph) pairs, in toolbar order. Shared by
/// `draw_canvas` header render (via `ui/mod.rs`) and the draw mouse hit-test
/// so they never drift — same pattern as `backup::render::backup_tabs`.
pub fn draw_tool_tabs(icon_mode: crate::config::IconMode) -> [(&'static str, &'static str); 5] {
    [
        (
            "Cursor",
            crate::ui::get_icon("\u{f245}", "\u{25b6}", icon_mode),
        ),
        (
            "Draw",
            crate::ui::get_icon("\u{f040}", "\u{270f}", icon_mode),
        ),
        (
            "Shape",
            crate::ui::get_icon("\u{f0c8}", "\u{25a0}", icon_mode),
        ),
        (
            "Text",
            crate::ui::get_icon("\u{f031}", "\u{1f4dd}", icon_mode),
        ),
        (
            "Erase",
            crate::ui::get_icon("\u{f1f8}", "\u{1f5d1}", icon_mode),
        ),
    ]
}
/// Tab order is fixed and intentionally not tied to `DrawTool` declaration
/// order. Keep this array as the single index-to-tool source of truth.
pub const DRAW_TAB_TOOLS: [DrawTool; 5] = [
    DrawTool::Cursor,
    DrawTool::Draw,
    DrawTool::Shape,
    DrawTool::Text,
    DrawTool::Erase,
];

/// Index of a tool within the tab order, for `build_tab_spans(active)`.
pub fn draw_tool_tab_index(tool: DrawTool) -> usize {
    DRAW_TAB_TOOLS.iter().position(|t| *t == tool).unwrap_or(0)
}

pub fn draw_canvas(
    frame: &mut Frame,
    app: &mut DrawAppState,
    area: Rect,
    config: &crate::config::ClinConfig,
    mouse_pos: Option<(u16, u16)>,
) {
    let mut canvas_area = area;
    canvas_area.height = canvas_area.height.saturating_sub(1);
    let x_bounds = [
        app.viewport.x - 100.0 / app.viewport.zoom,
        app.viewport.x + 100.0 / app.viewport.zoom,
    ];
    let y_bounds = [
        app.viewport.y - 100.0 / app.viewport.zoom,
        app.viewport.y + 100.0 / app.viewport.zoom,
    ];

    let cols_per_world_x =
        (canvas_area.width.saturating_sub(1) as f64) / (x_bounds[1] - x_bounds[0]);
    let rows_per_world_y =
        -(canvas_area.height.saturating_sub(1) as f64) / (y_bounds[1] - y_bounds[0]);

    let canvas = Canvas::default()
        .block(Block::default().style(Style::default().bg(app.theme.bg.unwrap_or(Color::Reset))))
        .background_color(app.theme.bg.unwrap_or(Color::Reset))
        .marker(Marker::Braille)
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .paint(|ctx| {
            for item in app.data.elements.iter().filter(|item| {
                !matches!(&item.element, DrawElement::Text(_))
                    && !is_interaction_item(app, &item.id)
            }) {
                draw_item(ctx, item);
            }

            if let Some(stroke) = &app.current_stroke {
                draw_smoothed_stroke(ctx, stroke);
            }

            if let Some(DrawElement::Shape(shape)) = &app.preview_element {
                draw_shape(ctx, shape, crate::draw::geometry::DrawAffine::identity());
            }
            draw_interaction_preview(ctx, app, false);

            for item in app.data.elements.iter().filter(|item| {
                matches!(&item.element, DrawElement::Text(_)) && !is_interaction_item(app, &item.id)
            }) {
                draw_item(ctx, item);
            }
            draw_interaction_preview(ctx, app, true);
            draw_selection_and_hover(ctx, app);
        });

    frame.render_widget(canvas, canvas_area);
    crate::ui::draw_canvas_grid(
        frame,
        canvas_area,
        app.grid,
        crate::ui::CanvasGridProjection {
            world_left: x_bounds[0],
            world_right: x_bounds[1],
            world_top: y_bounds[0],
            world_bottom: y_bounds[1],
            origin_col: canvas_area.left() as f64 - x_bounds[0] * cols_per_world_x,
            origin_row: canvas_area.top() as f64 - y_bounds[1] * rows_per_world_y,
            cols_per_world_x,
            rows_per_world_y,
        },
        app.theme.muted,
        app.viewport.zoom,
    );

    let status_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(1),
        area.width,
        1,
    );
    let hints_items = [
        (
            app.keybinds.display_draw(DrawAction::SelectCursorTool),
            "cursor",
        ),
        (
            app.keybinds.display_draw(DrawAction::SelectDrawTool),
            "draw",
        ),
        (
            app.keybinds.display_draw(DrawAction::ToggleShapeSelector),
            "shape",
        ),
        (
            app.keybinds.display_draw(DrawAction::SelectTextTool),
            "text",
        ),
        (
            app.keybinds.display_draw(DrawAction::SelectEraseTool),
            "erase",
        ),
        (app.keybinds.draw_keys_display(DrawAction::Quit), "back"),
        (
            format!("F1/{}", app.keybinds.draw_keys_display(DrawAction::Help)),
            "help",
        ),
        ("F2".to_string(), "keybinds"),
    ];
    let hint_line = crate::ui::format_keybind_hints(&app.theme, &hints_items);
    let mut ctx = crate::statusline::StatuslineContext::for_overlay(config, ViewMode::Draw);
    ctx.area = Some(status_area);
    ctx.draw = Some(app);
    ctx.hints = Some(hint_line.spans);
    if let Some(p) = &app.seq_matcher.pending_display() {
        ctx.pending = Some(vec![Span::styled(
            format!("{} ", p),
            Style::default()
                .fg(app.theme.highlight_fg)
                .bg(app.theme.accent),
        )]);
    }

    let (left_line, right_line) =
        crate::statusline::render_footer(&ctx, &config.statusline, ViewMode::Draw, &app.theme);
    crate::ui::draw_status_bar(frame, status_area, &app.theme, left_line, right_line);

    if let Some(menu) = &app.context_menu {
        crate::ui::render_canvas_context_menu(frame, canvas_area, menu, &app.theme, mouse_pos);
    }

    if app.show_shape_selector {
        let shapes = [
            (DrawShapeType::Rect, "Rect"),
            (DrawShapeType::Ellipse, "Ellipse"),
            (DrawShapeType::Diamond, "Diamond"),
            (DrawShapeType::Line, "Line"),
            (DrawShapeType::Arrow, "Arrow"),
        ];

        let items: Vec<(&str, bool, Option<ratatui::style::Color>)> = shapes
            .iter()
            .map(|(st, name)| (*name, app.active_shape_type == *st, None))
            .collect();

        let hints_array = [
            (
                app.keybinds
                    .display_draw(crate::keybinds::DrawAction::ShapeSelectorConfirm),
                "select",
            ),
            (
                app.keybinds
                    .display_draw(crate::keybinds::DrawAction::ShapeSelectorCancel),
                "cancel",
            ),
        ];

        crate::ui::draw_header_dropdown(
            frame,
            area,
            "SELECT SHAPE",
            &items,
            mouse_pos,
            Some(crate::ui::PopupHints::Keybinds(&hints_array)),
            &app.theme,
        );
    }

    if app.show_color_selector {
        let colors = crate::pinstar::COLOR_PICKER_PALETTE;
        let items: Vec<(&str, bool, Option<ratatui::style::Color>)> = colors
            .iter()
            .map(|(name, _, color)| {
                let is_active = if let ratatui::style::Color::Rgb(r, g, b) = *color {
                    (r, g, b) == app.active_color
                } else {
                    false
                };
                (*name, is_active, Some(*color))
            })
            .collect();

        let hints_array = [
            (
                app.keybinds
                    .display_draw(crate::keybinds::DrawAction::ColorSelectorConfirm),
                "select",
            ),
            (
                app.keybinds
                    .display_draw(crate::keybinds::DrawAction::ColorSelectorCancel),
                "cancel",
            ),
        ];

        crate::ui::draw_header_dropdown(
            frame,
            area,
            "SELECT COLOR",
            &items,
            mouse_pos,
            Some(crate::ui::PopupHints::Keybinds(&hints_array)),
            &app.theme,
        );
    }
    app.text_editor_rect = None;

    if let Some((_, textarea)) = &app.text_editor {
        let content = crate::ui::draw_popup_frame(
            frame,
            area,
            "EDIT TEXT",
            crate::ui::PopupSize::Prompt,
            crate::ui::PopupHints::Keybinds(&[
                (
                    app.keybinds
                        .display_draw(crate::keybinds::DrawAction::TextEditorConfirm),
                    "save",
                ),
                (
                    app.keybinds
                        .display_draw(crate::keybinds::DrawAction::TextEditorCancel),
                    "cancel",
                ),
            ]),
            &app.theme,
        );

        let mut themed_textarea = textarea.clone();
        themed_textarea.set_block(
            Block::bordered()
                .style(app.theme.bg_style())
                .border_style(Style::default().fg(app.theme.accent)),
        );
        app.text_editor_rect = Some(Block::bordered().inner(content));
        themed_textarea.set_style(app.theme.bg_style());

        frame.render_widget(&themed_textarea, content);
    }
}

fn is_interaction_item(app: &DrawAppState, item_id: &crate::draw::state::DrawItemId) -> bool {
    match &app.interaction {
        Some(DrawInteraction::Move { id, .. })
        | Some(DrawInteraction::Rotate { id, .. })
        | Some(DrawInteraction::Scale { id, .. }) => id == item_id,
        Some(DrawInteraction::Paste { .. }) | None => false,
    }
}

fn interaction_transform_for_item(
    item: &DrawItem,
    interaction: Option<&DrawInteraction>,
) -> DrawTransform {
    let mut transform = item.transform;
    match interaction {
        Some(DrawInteraction::Move {
            id,
            preview_translation,
            ..
        }) if id == &item.id => {
            transform.translate_x = preview_translation.0;
            transform.translate_y = preview_translation.1;
        }
        Some(DrawInteraction::Rotate {
            id,
            preview_degrees,
            ..
        }) if id == &item.id => {
            transform.rotation_degrees = *preview_degrees;
        }
        Some(DrawInteraction::Scale {
            id, preview_scale, ..
        }) if id == &item.id => {
            transform.scale = *preview_scale;
        }
        Some(DrawInteraction::Paste { .. }) | None => {}
        _ => {}
    }
    transform
}

fn draw_interaction_preview(ctx: &mut Context, app: &DrawAppState, text: bool) {
    let Some(interaction) = &app.interaction else {
        return;
    };
    match interaction {
        DrawInteraction::Paste { item } => {
            if matches!(&item.element, DrawElement::Text(_)) == text {
                draw_item(ctx, item);
            }
        }
        DrawInteraction::Move { id, .. }
        | DrawInteraction::Rotate { id, .. }
        | DrawInteraction::Scale { id, .. } => {
            let Some(item) = app.data.item(id) else {
                return;
            };
            if matches!(&item.element, DrawElement::Text(_)) == text {
                draw_item_with_transform(
                    ctx,
                    item,
                    interaction_transform_for_item(item, app.interaction.as_ref()),
                );
            }
        }
    }
}

fn draw_selection_and_hover(ctx: &mut Context, app: &DrawAppState) {
    if let Some(id) = &app.selection.primary
        && let Some(item) = app.data.item(id)
    {
        let transform = interaction_transform_for_item(item, app.interaction.as_ref());
        draw_item_with_transform_and_color(
            ctx,
            item,
            transform,
            blended_item_color(item, app.theme.accent, 45),
        );
        draw_selection_bounds(ctx, item, transform, app);
    } else if let Some(id) = &app.hovered
        && let Some(item) = app.data.item(id)
    {
        let transform = interaction_transform_for_item(item, app.interaction.as_ref());
        draw_item_with_transform_and_color(
            ctx,
            item,
            transform,
            blended_item_color(item, app.theme.accent, 30),
        );
    }
}

fn draw_selection_bounds(
    ctx: &mut Context,
    item: &DrawItem,
    transform: DrawTransform,
    app: &DrawAppState,
) {
    let Some(bounds) = crate::draw::geometry::transformed_bounds_with_transform(item, &transform)
    else {
        return;
    };
    let style = Style::default().fg(app.theme.accent);
    ctx.print(
        bounds.min_x,
        bounds.max_y,
        ratatui::text::Line::from("┌").style(style),
    );
    ctx.print(
        bounds.max_x,
        bounds.max_y,
        ratatui::text::Line::from("┐").style(style),
    );
    ctx.print(
        bounds.min_x,
        bounds.min_y,
        ratatui::text::Line::from("└").style(style),
    );
    ctx.print(
        bounds.max_x,
        bounds.min_y,
        ratatui::text::Line::from("┘").style(style),
    );
    let active_handle = match app.interaction.as_ref() {
        Some(DrawInteraction::Rotate { id, .. }) if id == &item.id => {
            crate::draw::geometry::selection_handle_points(item, &transform, &app.viewport)
                .map(|(rotation, _)| (rotation, "○"))
        }
        Some(DrawInteraction::Scale { id, .. }) if id == &item.id => {
            crate::draw::geometry::selection_handle_points(item, &transform, &app.viewport)
                .map(|(_, scale)| (scale, "◢"))
        }
        _ => None,
    };
    if let Some((point, glyph)) = active_handle {
        ctx.print(
            point.0,
            point.1,
            ratatui::text::Line::from(glyph).style(style),
        );
    }
}

pub(crate) fn draw_item(ctx: &mut Context, item: &DrawItem) {
    draw_item_with_transform(ctx, item, item.transform);
}

fn draw_item_with_transform(ctx: &mut Context, item: &DrawItem, transform: DrawTransform) {
    draw_item_with_transform_and_color(ctx, item, transform, item_color(item));
}

fn draw_item_with_transform_and_color(
    ctx: &mut Context,
    item: &DrawItem,
    transform: DrawTransform,
    color: Color,
) {
    match &item.element {
        DrawElement::Stroke(stroke) => {
            draw_stroke_colored(
                ctx,
                stroke,
                crate::draw::geometry::DrawAffine::new(&transform),
                color,
            );
        }
        DrawElement::Shape(shape) => {
            draw_shape_colored(
                ctx,
                shape,
                crate::draw::geometry::DrawAffine::new(&transform),
                color,
            );
        }
        DrawElement::Text(text) => {
            ctx.print(
                text.x + transform.translate_x,
                text.y + transform.translate_y,
                ratatui::text::Line::from(text.content.clone()).style(Style::default().fg(color)),
            );
        }
    }
}

fn item_color(item: &DrawItem) -> Color {
    match &item.element {
        DrawElement::Stroke(stroke) => Color::Rgb(stroke.color.0, stroke.color.1, stroke.color.2),
        DrawElement::Shape(
            Shape::Rect { color, .. }
            | Shape::Ellipse { color, .. }
            | Shape::Diamond { color, .. }
            | Shape::Line { color, .. }
            | Shape::Arrow { color, .. },
        ) => Color::Rgb(color.0, color.1, color.2),
        DrawElement::Text(text) => Color::Rgb(text.color.0, text.color.1, text.color.2),
    }
}

fn blended_item_color(item: &DrawItem, accent: Color, amount: u8) -> Color {
    let Color::Rgb(red, green, blue) = item_color(item) else {
        return accent;
    };
    let Some((accent_red, accent_green, accent_blue)) = color_rgb(accent) else {
        return accent;
    };
    let amount = u16::from(amount);
    let inverse = 100 - amount;
    Color::Rgb(
        ((u16::from(red) * inverse + u16::from(accent_red) * amount) / 100) as u8,
        ((u16::from(green) * inverse + u16::from(accent_green) * amount) / 100) as u8,
        ((u16::from(blue) * inverse + u16::from(accent_blue) * amount) / 100) as u8,
    )
}

fn color_rgb(color: Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Black => Some((0, 0, 0)),
        Color::Red => Some((128, 0, 0)),
        Color::Green => Some((0, 128, 0)),
        Color::Yellow => Some((128, 128, 0)),
        Color::Blue => Some((0, 0, 128)),
        Color::Magenta => Some((128, 0, 128)),
        Color::Cyan => Some((0, 128, 128)),
        Color::Gray => Some((192, 192, 192)),
        Color::DarkGray => Some((128, 128, 128)),
        Color::LightRed => Some((255, 0, 0)),
        Color::LightGreen => Some((0, 255, 0)),
        Color::LightYellow => Some((255, 255, 0)),
        Color::LightBlue => Some((0, 0, 255)),
        Color::LightMagenta => Some((255, 0, 255)),
        Color::LightCyan => Some((0, 255, 255)),
        Color::White => Some((255, 255, 255)),
        Color::Rgb(red, green, blue) => Some((red, green, blue)),
        Color::Reset | Color::Indexed(_) => None,
    }
}

fn draw_stroke(ctx: &mut Context, stroke: &Stroke, transform: crate::draw::geometry::DrawAffine) {
    draw_stroke_colored(
        ctx,
        stroke,
        transform,
        Color::Rgb(stroke.color.0, stroke.color.1, stroke.color.2),
    );
}

fn draw_stroke_colored(
    ctx: &mut Context,
    stroke: &Stroke,
    transform: crate::draw::geometry::DrawAffine,
    color: Color,
) {
    for window in stroke.points.windows(2) {
        if let [start, end] = window {
            draw_transformed_line(ctx, *start, *end, color, transform);
        }
    }
}

fn draw_canvas_line(ctx: &mut Context, start: (f64, f64), end: (f64, f64), color: Color) {
    ctx.draw(&Line {
        x1: start.0,
        y1: start.1,
        x2: end.0,
        y2: end.1,
        color,
    });
}

fn draw_transformed_line(
    ctx: &mut Context,
    start: (f64, f64),
    end: (f64, f64),
    color: Color,
    transform: crate::draw::geometry::DrawAffine,
) {
    draw_canvas_line(
        ctx,
        transform.transform_point(start),
        transform.transform_point(end),
        color,
    );
}

fn draw_closed_polygon(
    ctx: &mut Context,
    points: &[(f64, f64)],
    color: Color,
    transform: crate::draw::geometry::DrawAffine,
) {
    let Some((&first, rest)) = points.split_first() else {
        return;
    };
    let mut previous = transform.transform_point(first);
    for &point in rest {
        let current = transform.transform_point(point);
        draw_canvas_line(ctx, previous, current, color);
        previous = current;
    }
    draw_canvas_line(ctx, previous, transform.transform_point(first), color);
}

/// Binomial filter smoothing (discrete Gaussian blur).
/// Applies a 3-point moving average with weights [0.25, 0.5, 0.25]
/// for 10 iterations. Acts as a powerful low-pass filter that
/// eliminates stair-step quantization noise.
pub fn smooth_points(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let n = points.len();
    let mut xs: Vec<f64> = points.iter().map(|point| point.0).collect();
    let mut ys: Vec<f64> = points.iter().map(|point| point.1).collect();
    for _ in 0..10 {
        let previous_xs = xs.clone();
        let previous_ys = ys.clone();
        xs[0] = previous_xs[0];
        ys[0] = previous_ys[0];
        for index in 1..n - 1 {
            xs[index] = 0.25 * previous_xs[index - 1]
                + 0.5 * previous_xs[index]
                + 0.25 * previous_xs[index + 1];
            ys[index] = 0.25 * previous_ys[index - 1]
                + 0.5 * previous_ys[index]
                + 0.25 * previous_ys[index + 1];
        }
        xs[n - 1] = previous_xs[n - 1];
        ys[n - 1] = previous_ys[n - 1];
    }
    xs.into_iter().zip(ys).collect()
}

fn draw_smoothed_stroke(ctx: &mut Context, stroke: &Stroke) {
    let smoothed = smooth_points(&stroke.points);
    draw_stroke(
        ctx,
        &Stroke {
            points: smoothed,
            color: stroke.color,
        },
        crate::draw::geometry::DrawAffine::identity(),
    );
}

fn draw_shape(ctx: &mut Context, shape: &Shape, transform: crate::draw::geometry::DrawAffine) {
    draw_shape_colored(ctx, shape, transform, shape_color(shape));
}

fn shape_color(shape: &Shape) -> Color {
    match shape {
        Shape::Rect { color, .. }
        | Shape::Ellipse { color, .. }
        | Shape::Diamond { color, .. }
        | Shape::Line { color, .. }
        | Shape::Arrow { color, .. } => Color::Rgb(color.0, color.1, color.2),
    }
}

fn draw_shape_colored(
    ctx: &mut Context,
    shape: &Shape,
    transform: crate::draw::geometry::DrawAffine,
    draw_color: Color,
) {
    match shape {
        Shape::Rect {
            x,
            y,
            width,
            height,
            ..
        } if transform.is_identity() => {
            ctx.draw(&Rectangle {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                color: draw_color,
            });
        }
        Shape::Rect {
            x,
            y,
            width,
            height,
            ..
        } => {
            let points = [
                (*x, *y),
                (*x + *width, *y),
                (*x + *width, *y + *height),
                (*x, *y + *height),
            ];
            draw_closed_polygon(ctx, &points, draw_color, transform);
        }
        Shape::Ellipse {
            x,
            y,
            width,
            height,
            ..
        } => {
            let radius_x = width / 2.0;
            let radius_y = height / 2.0;
            let center_x = x + radius_x;
            let center_y = y + radius_y;
            const SEGMENTS: usize = 32;
            for index in 0..SEGMENTS {
                let start_angle = (index as f64 / SEGMENTS as f64) * std::f64::consts::TAU;
                let end_angle = ((index + 1) as f64 / SEGMENTS as f64) * std::f64::consts::TAU;
                draw_transformed_line(
                    ctx,
                    (
                        center_x + radius_x * start_angle.cos(),
                        center_y + radius_y * start_angle.sin(),
                    ),
                    (
                        center_x + radius_x * end_angle.cos(),
                        center_y + radius_y * end_angle.sin(),
                    ),
                    draw_color,
                    transform,
                );
            }
        }
        Shape::Diamond {
            x,
            y,
            width,
            height,
            ..
        } => {
            let points = [
                (*x + *width / 2.0, *y),
                (*x + *width, *y + *height / 2.0),
                (*x + *width / 2.0, *y + *height),
                (*x, *y + *height / 2.0),
            ];
            draw_closed_polygon(ctx, &points, draw_color, transform);
        }
        Shape::Line { x1, y1, x2, y2, .. } => {
            draw_transformed_line(ctx, (*x1, *y1), (*x2, *y2), draw_color, transform);
        }
        Shape::Arrow { x1, y1, x2, y2, .. } => {
            let start = (*x1, *y1);
            let end = (*x2, *y2);
            draw_transformed_line(ctx, start, end, draw_color, transform);

            let angle = (y2 - y1).atan2(x2 - x1);
            let head_length = 5.0;
            let head_angle = std::f64::consts::PI / 6.0;
            let left = (
                x2 - head_length * (angle - head_angle).cos(),
                y2 - head_length * (angle - head_angle).sin(),
            );
            let right = (
                x2 - head_length * (angle + head_angle).cos(),
                y2 - head_length * (angle + head_angle).sin(),
            );
            draw_transformed_line(ctx, end, left, draw_color, transform);
            draw_transformed_line(ctx, end, right, draw_color, transform);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_and_selection_blend_item_color_toward_accent() {
        let item = DrawItem::new(DrawElement::Stroke(Stroke {
            points: vec![(0.0, 0.0)],
            color: (100, 150, 200),
        }));
        let accent = Color::Rgb(200, 100, 0);

        assert_eq!(
            blended_item_color(&item, accent, 30),
            Color::Rgb(130, 135, 140)
        );
        assert_eq!(
            blended_item_color(&item, accent, 45),
            Color::Rgb(145, 127, 110)
        );
    }
}
