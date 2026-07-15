use crate::app::ViewMode;
use crate::draw::app::DrawAppState;
use crate::draw::state::{DrawElement, DrawShapeType, DrawTool, Shape, Stroke};
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
pub fn draw_tool_tabs(icon_mode: crate::config::IconMode) -> [(&'static str, &'static str); 4] {
    [
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
/// Tab order is fixed (Draw, Shape, Text, Erase) and intentionally NOT the
/// `DrawTool` enum ordinal (enum is Draw, Erase, Text, Shape). Keep this array
/// the single source of truth for index<->tool.
pub const DRAW_TAB_TOOLS: [DrawTool; 4] = [
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
            for element in &app.data.elements {
                match element {
                    DrawElement::Stroke(stroke) => {
                        draw_stroke(ctx, stroke);
                    }
                    DrawElement::Shape(shape) => {
                        draw_shape(ctx, shape);
                    }
                    DrawElement::Text(text) => {
                        let content = text.content.clone();
                        let color = Color::Rgb(text.color.0, text.color.1, text.color.2);
                        ctx.print(
                            text.x,
                            text.y,
                            ratatui::text::Line::from(content).style(Style::default().fg(color)),
                        );
                    }
                    DrawElement::Image(_) => {
                        // Rendered as StatefulImage pass after the canvas widget
                    }
                }
            }

            if let Some(stroke) = &app.current_stroke {
                draw_smoothed_stroke(ctx, stroke);
            }

            if let Some(DrawElement::Shape(shape)) = &app.preview_element {
                draw_shape(ctx, shape);
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
        (app.keybinds.display_draw(DrawAction::Quit), "back"),
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
        themed_textarea.set_style(app.theme.bg_style());

        frame.render_widget(&themed_textarea, content);
    }
}

fn draw_stroke(ctx: &mut Context, stroke: &Stroke) {
    let color = Color::Rgb(stroke.color.0, stroke.color.1, stroke.color.2);
    for window in stroke.points.windows(2) {
        if let [p1, p2] = window {
            ctx.draw(&Line {
                x1: p1.0,
                y1: p1.1,
                x2: p2.0,
                y2: p2.1,
                color,
            });
        }
    }
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
    let mut xs: Vec<f64> = points.iter().map(|p| p.0).collect();
    let mut ys: Vec<f64> = points.iter().map(|p| p.1).collect();
    for _ in 0..10 {
        let prev_xs = xs.clone();
        let prev_ys = ys.clone();
        xs[0] = prev_xs[0];
        ys[0] = prev_ys[0];
        for i in 1..n - 1 {
            xs[i] = 0.25 * prev_xs[i - 1] + 0.5 * prev_xs[i] + 0.25 * prev_xs[i + 1];
            ys[i] = 0.25 * prev_ys[i - 1] + 0.5 * prev_ys[i] + 0.25 * prev_ys[i + 1];
        }
        xs[n - 1] = prev_xs[n - 1];
        ys[n - 1] = prev_ys[n - 1];
    }
    xs.into_iter().zip(ys).collect()
}

/// Draw a stroke after applying binomial smoothing.
fn draw_smoothed_stroke(ctx: &mut Context, stroke: &Stroke) {
    let smoothed = smooth_points(&stroke.points);
    let color = Color::Rgb(stroke.color.0, stroke.color.1, stroke.color.2);
    for window in smoothed.windows(2) {
        if let [p1, p2] = window {
            ctx.draw(&Line {
                x1: p1.0,
                y1: p1.1,
                x2: p2.0,
                y2: p2.1,
                color,
            });
        }
    }
}

fn draw_shape(ctx: &mut Context, shape: &Shape) {
    match shape {
        Shape::Rect {
            x,
            y,
            width,
            height,
            color,
        } => {
            ctx.draw(&Rectangle {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                color: Color::Rgb(color.0, color.1, color.2),
            });
        }
        Shape::Ellipse {
            x,
            y,
            width,
            height,
            color,
        } => {
            let color = Color::Rgb(color.0, color.1, color.2);
            let rx = width / 2.0;
            let ry = height / 2.0;
            let cx_center = x + rx;
            let cy_center = y + ry;
            let segments = 32;
            for i in 0..segments {
                let angle1 = (i as f64 / segments as f64) * 2.0 * std::f64::consts::PI;
                let angle2 = ((i + 1) as f64 / segments as f64) * 2.0 * std::f64::consts::PI;
                ctx.draw(&Line {
                    x1: cx_center + rx * angle1.cos(),
                    y1: cy_center + ry * angle1.sin(),
                    x2: cx_center + rx * angle2.cos(),
                    y2: cy_center + ry * angle2.sin(),
                    color,
                });
            }
        }
        Shape::Diamond {
            x,
            y,
            width,
            height,
            color,
        } => {
            let color = Color::Rgb(color.0, color.1, color.2);
            let p1 = (x + width / 2.0, *y);
            let p2 = (x + width, y + height / 2.0);
            let p3 = (x + width / 2.0, y + height);
            let p4 = (*x, y + height / 2.0);

            for (start, end) in [(p1, p2), (p2, p3), (p3, p4), (p4, p1)] {
                ctx.draw(&Line {
                    x1: start.0,
                    y1: start.1,
                    x2: end.0,
                    y2: end.1,
                    color,
                });
            }
        }
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            color,
        } => {
            ctx.draw(&Line {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                color: Color::Rgb(color.0, color.1, color.2),
            });
        }
        Shape::Arrow {
            x1,
            y1,
            x2,
            y2,
            color,
        } => {
            let color = Color::Rgb(color.0, color.1, color.2);
            ctx.draw(&Line {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                color,
            });

            let angle = (y2 - y1).atan2(x2 - x1);
            let head_len = 5.0;
            let head_angle = std::f64::consts::PI / 6.0;

            ctx.draw(&Line {
                x1: *x2,
                y1: *y2,
                x2: x2 - head_len * (angle - head_angle).cos(),
                y2: y2 - head_len * (angle - head_angle).sin(),
                color,
            });
            ctx.draw(&Line {
                x1: *x2,
                y1: *y2,
                x2: x2 - head_len * (angle + head_angle).cos(),
                y2: y2 - head_len * (angle + head_angle).sin(),
                color,
            });
        }
    }
}
