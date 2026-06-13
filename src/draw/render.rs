use crate::constants::DRAW_HELP_HINTS;
use crate::draw::app::DrawAppState;
use crate::draw::state::{DrawElement, DrawShapeType, DrawTool, Shape, Stroke};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line as TuiLine, Span};
use ratatui::widgets::canvas::{Canvas, Context, Line, Rectangle};
use ratatui::widgets::{Block, List, ListItem, Paragraph};

pub fn draw_canvas(frame: &mut Frame, app: &DrawAppState, area: Rect) {

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
                }
            }

            if let Some(stroke) = &app.current_stroke {
                draw_stroke(ctx, stroke);
            }

            if let Some(DrawElement::Shape(shape)) = &app.preview_element {
                draw_shape(ctx, shape);
            }
        });

    frame.render_widget(canvas, area);

    let toolbar_width = 42;
    let toolbar_area = Rect::new(
        area.x + area.width.saturating_sub(toolbar_width) / 2,
        area.y + area.height.saturating_sub(2),
        toolbar_width,
        1,
    );

    let tools = [
        (DrawTool::Draw, "\u{f040} Draw"),
        (DrawTool::Shape, "\u{f0c8} Shape"),
        (DrawTool::Text, "\u{f031} Text"),
        (DrawTool::Erase, "\u{f1f8} Erase"),
    ];

    let mut spans = Vec::new();
    for (i, (tool, label)) in tools.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        let style = if app.active_tool == *tool {
            Style::default()
                .fg(app.theme.highlight_fg)
                .bg(app.theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg)
        };
        spans.push(Span::styled(*label, style));
    }

    frame.render_widget(
        Paragraph::new(TuiLine::from(spans)).alignment(Alignment::Center),
        toolbar_area,
    );

    let status_area = Rect::new(area.x, area.y + area.height.saturating_sub(1), area.width, 1);
    crate::ui::draw_status_bar(frame, status_area, &app.theme, None, DRAW_HELP_HINTS, None);

    if app.show_shape_selector {
        let content = crate::ui::draw_popup_frame(
            frame,
            area,
            "SELECT SHAPE",
            30,
            40,
            "Enter select · Esc cancel",
            &app.theme,
        );

        let shapes = [
            (DrawShapeType::Rect, "Rect"),
            (DrawShapeType::Ellipse, "Ellipse"),
            (DrawShapeType::Diamond, "Diamond"),
            (DrawShapeType::Line, "Line"),
            (DrawShapeType::Arrow, "Arrow"),
        ];

        let items: Vec<ListItem> = shapes
            .iter()
            .map(|(st, name)| {
                let style = if app.active_shape_type == *st {
                    Style::default()
                        .fg(app.theme.highlight_fg)
                        .bg(app.theme.highlight_bg)
                } else {
                    Style::default().fg(app.theme.fg)
                };
                ListItem::new(format!("  {}", name)).style(style)
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
            50,
            10,
            "Enter save · Esc cancel",
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
