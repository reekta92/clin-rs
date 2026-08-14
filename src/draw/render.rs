use crate::app::ViewMode;
use crate::draw::app::DrawAppState;
use crate::draw::state::{DrawElement, DrawItem, DrawShapeType, DrawTool, Shape, Stroke};
use crate::keybinds::DrawAction;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::text::Span;
use ratatui::widgets::canvas::{Canvas, Context, Line, Rectangle};
use ratatui::widgets::{Block, List, ListItem};

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

    let canvas = Canvas::default()
        .block(Block::default().style(Style::default().bg(app.theme.bg.unwrap_or(Color::Reset))))
        .background_color(app.theme.bg.unwrap_or(Color::Reset))
        .marker(Marker::Braille)
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .paint(|ctx| {
            if app.show_grid {
                let mut grid_step_x = 100.0;
                let mut grid_step_y = 100.0;
                while grid_step_y * app.viewport.zoom < 6.0 {
                    grid_step_x *= 2.0;
                    grid_step_y *= 2.0;
                }
                // compensate for terminal cell aspect ratio (~2:1 height:width) so grid appears even
                grid_step_y *= canvas_area.width as f64 / (2.0 * canvas_area.height as f64);
                let start_x = (x_bounds[0] / grid_step_x).floor() * grid_step_x;
                let end_x = (x_bounds[1] / grid_step_x).ceil() * grid_step_x;
                let start_y = (y_bounds[0] / grid_step_y).floor() * grid_step_y;
                let end_y = (y_bounds[1] / grid_step_y).ceil() * grid_step_y;
                let mut cur_x = start_x;
                while cur_x <= end_x {
                    let mut cur_y = start_y;
                    while cur_y <= end_y {
                        ctx.print(
                            cur_x,
                            cur_y,
                            ratatui::text::Line::from("·")
                                .style(Style::default().fg(app.theme.muted)),
                        );
                        cur_y += grid_step_y;
                    }
                    cur_x += grid_step_x;
                }
            }
            for item in app
                .data
                .elements
                .iter()
                .filter(|item| !matches!(&item.element, DrawElement::Text(_)))
            {
                draw_item(ctx, item);
            }

            if let Some(stroke) = &app.current_stroke {
                draw_smoothed_stroke(ctx, stroke);
            }

            if let Some(DrawElement::Shape(shape)) = &app.preview_element {
                draw_shape(ctx, shape, crate::draw::geometry::DrawAffine::identity());
            }

            for item in app
                .data
                .elements
                .iter()
                .filter(|item| matches!(&item.element, DrawElement::Text(_)))
            {
                draw_item(ctx, item);
            }
        });

    frame.render_widget(canvas, canvas_area);

    let status_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(1),
        area.width,
        1,
    );
    let hints_items = vec![
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
        (app.keybinds.display_draw(DrawAction::ToggleGrid), "grid"),
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

    if app.show_shape_selector {
        let content = crate::ui::draw_popup_frame(
            frame,
            area,
            "SELECT SHAPE",
            crate::ui::PopupSize::Small,
            crate::ui::PopupHints::Keybinds(&[
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
            ]),
            &app.theme,
        );

        let shapes = [
            (DrawShapeType::Rect, "Rect"),
            (DrawShapeType::Ellipse, "Ellipse"),
            (DrawShapeType::Diamond, "Diamond"),
            (DrawShapeType::Line, "Line"),
            (DrawShapeType::Arrow, "Arrow"),
        ];
        let hovered_idx = mouse_pos.and_then(|(col, row)| {
            let items_top = content.y + 1;
            let items_bottom = items_top + shapes.len() as u16;
            if row >= items_top
                && row < items_bottom
                && col > content.x
                && col < content.x + content.width - 1
            {
                Some((row - items_top) as usize)
            } else {
                None
            }
        });

        let items: Vec<ListItem> = shapes
            .iter()
            .enumerate()
            .map(|(i, (st, name))| {
                let style = if app.active_shape_type == *st {
                    Style::default()
                        .fg(app.theme.highlight_fg)
                        .bg(app.theme.highlight_bg)
                } else if hovered_idx == Some(i) {
                    app.theme.hover_style()
                } else {
                    Style::default().fg(app.theme.fg)
                };
                ListItem::new(format!("  {name}")).style(style)
            })
            .collect();

        let list = List::new(items).block(
            Block::bordered()
                .border_style(Style::default().fg(app.theme.accent))
                .style(app.theme.bg_style()),
        );

        frame.render_widget(list, content);
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

pub(crate) fn draw_item(ctx: &mut Context, item: &DrawItem) {
    match &item.element {
        DrawElement::Stroke(stroke) => {
            draw_stroke(
                ctx,
                stroke,
                crate::draw::geometry::DrawAffine::new(&item.transform),
            );
        }
        DrawElement::Shape(shape) => {
            draw_shape(
                ctx,
                shape,
                crate::draw::geometry::DrawAffine::new(&item.transform),
            );
        }
        DrawElement::Text(text) => {
            if let Some((x, y)) = crate::draw::geometry::translated_text_position(item) {
                let color = Color::Rgb(text.color.0, text.color.1, text.color.2);
                ctx.print(
                    x,
                    y,
                    ratatui::text::Line::from(text.content.clone())
                        .style(Style::default().fg(color)),
                );
            }
        }
    }
}

fn draw_stroke(ctx: &mut Context, stroke: &Stroke, transform: crate::draw::geometry::DrawAffine) {
    let color = Color::Rgb(stroke.color.0, stroke.color.1, stroke.color.2);
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
    match shape {
        Shape::Rect {
            x,
            y,
            width,
            height,
            color,
        } if transform.is_identity() => {
            ctx.draw(&Rectangle {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                color: Color::Rgb(color.0, color.1, color.2),
            });
        }
        Shape::Rect {
            x,
            y,
            width,
            height,
            color,
        } => {
            let points = [
                (*x, *y),
                (*x + *width, *y),
                (*x + *width, *y + *height),
                (*x, *y + *height),
            ];
            draw_closed_polygon(
                ctx,
                &points,
                Color::Rgb(color.0, color.1, color.2),
                transform,
            );
        }
        Shape::Ellipse {
            x,
            y,
            width,
            height,
            color,
        } => {
            let color = Color::Rgb(color.0, color.1, color.2);
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
                    color,
                    transform,
                );
            }
        }
        Shape::Diamond {
            x,
            y,
            width,
            height,
            color,
        } => {
            let points = [
                (*x + *width / 2.0, *y),
                (*x + *width, *y + *height / 2.0),
                (*x + *width / 2.0, *y + *height),
                (*x, *y + *height / 2.0),
            ];
            draw_closed_polygon(
                ctx,
                &points,
                Color::Rgb(color.0, color.1, color.2),
                transform,
            );
        }
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            color,
        } => {
            draw_transformed_line(
                ctx,
                (*x1, *y1),
                (*x2, *y2),
                Color::Rgb(color.0, color.1, color.2),
                transform,
            );
        }
        Shape::Arrow {
            x1,
            y1,
            x2,
            y2,
            color,
        } => {
            let color = Color::Rgb(color.0, color.1, color.2);
            let start = (*x1, *y1);
            let end = (*x2, *y2);
            draw_transformed_line(ctx, start, end, color, transform);

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
            draw_transformed_line(ctx, end, left, color, transform);
            draw_transformed_line(ctx, end, right, color, transform);
        }
    }
}
