use crate::draw::app::{DrawAppState, DrawEventAction};
use crate::draw::state::{DrawElement, DrawShapeType, DrawTool, Shape, Stroke, Text};
use crate::keybinds::Keybinds;
use crossterm::event::{Event, KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui_textarea::TextArea;

pub fn handle_event(
    ev: Event,
    app: &mut DrawAppState,
    _keybinds: &Keybinds,
) -> anyhow::Result<Option<DrawEventAction>> {
    if let Some((idx, textarea)) = &mut app.text_editor {
        match ev {
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) => {
                app.text_editor = None;
                return Ok(None);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                ..
            }) => {
                let new_content = textarea.lines()[0].clone();
                if let Some(DrawElement::Text(t)) = app.data.elements.get_mut(*idx) {
                    t.content = new_content;
                }
                app.text_editor = None;
                return Ok(Some(DrawEventAction::Save));
            }
            _ => {
                textarea.input(ev);
                return Ok(None);
            }
        }
    }

    if app.show_shape_selector {
        match ev {
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) => {
                app.show_shape_selector = false;
                return Ok(None);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                ..
            }) => {
                app.show_shape_selector = false;
                app.active_tool = DrawTool::Shape;
                return Ok(None);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Up, ..
            }) => {
                cycle_shape_type(app, -1);
                return Ok(None);
            }
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                ..
            }) => {
                cycle_shape_type(app, 1);
                return Ok(None);
            }
            _ => {}
        }
    }

    match ev {
        Event::Key(KeyEvent {
            code: KeyCode::Esc, ..
        }) => Ok(Some(DrawEventAction::Quit)),
        Event::Key(KeyEvent {
            code: KeyCode::Char('d'),
            ..
        }) => {
            app.active_tool = DrawTool::Draw;
            Ok(None)
        }
        Event::Key(KeyEvent {
            code: KeyCode::Char('s'),
            ..
        }) => {
            app.show_shape_selector = !app.show_shape_selector;
            Ok(None)
        }
        Event::Key(KeyEvent {
            code: KeyCode::Char('t'),
            ..
        }) => {
            app.active_tool = DrawTool::Text;
            Ok(None)
        }
        Event::Key(KeyEvent {
            code: KeyCode::Char('e'),
            ..
        }) => {
            app.active_tool = DrawTool::Erase;
            Ok(None)
        }
        Event::Mouse(mouse_event) => handle_mouse(mouse_event, app),
        _ => Ok(None),
    }
}

fn cycle_shape_type(app: &mut DrawAppState, delta: i32) {
    let shapes = [
        DrawShapeType::Rect,
        DrawShapeType::Ellipse,
        DrawShapeType::Diamond,
        DrawShapeType::Line,
        DrawShapeType::Arrow,
    ];
    let current_idx = shapes
        .iter()
        .position(|&s| s == app.active_shape_type)
        .unwrap_or(0) as i32;
    let next_idx = (current_idx + delta).rem_euclid(shapes.len() as i32) as usize;
    app.active_shape_type = shapes[next_idx];
}

fn handle_mouse(ev: MouseEvent, app: &mut DrawAppState) -> anyhow::Result<Option<DrawEventAction>> {
    let area = app.last_area;

    if app.show_shape_selector {
        let popup_width = 20;
        let popup_height = 7;
        let px = (area.width.saturating_sub(popup_width)) / 2;
        let py = (area.height.saturating_sub(popup_height)) / 2;

        if ev.kind == MouseEventKind::Down(MouseButton::Left) {
            if ev.column >= px
                && ev.column < px + popup_width
                && ev.row > py
                && ev.row < py + popup_height
            {
                let row_rel = (ev.row - py - 1) as usize;
                let shapes = [
                    DrawShapeType::Rect,
                    DrawShapeType::Ellipse,
                    DrawShapeType::Diamond,
                    DrawShapeType::Line,
                    DrawShapeType::Arrow,
                ];
                if let Some(&st) = shapes.get(row_rel) {
                    app.active_shape_type = st;
                    app.active_tool = DrawTool::Shape;
                    app.show_shape_selector = false;
                    return Ok(None);
                }
            } else {
                app.show_shape_selector = false;
            }
        }
        return Ok(None);
    }

    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let toolbar_width = 42;
            let tx = area.width.saturating_sub(toolbar_width) / 2;
            let ty = area.height.saturating_sub(1);

            if ev.row == ty && ev.column >= tx && ev.column < tx + toolbar_width {
                let col_rel = ev.column - tx;
                if col_rel < 10 {
                    app.active_tool = DrawTool::Draw;
                } else if col_rel < 21 {
                    app.show_shape_selector = true;
                } else if col_rel < 32 {
                    app.active_tool = DrawTool::Text;
                } else {
                    app.active_tool = DrawTool::Erase;
                }
                return Ok(None);
            }

            let (cx, cy) = screen_to_canvas(ev.column, ev.row, app);

            match app.active_tool {
                DrawTool::Draw => {
                    app.current_stroke = Some(Stroke {
                        points: vec![(cx, cy)],
                        color: (255, 255, 255),
                    });
                }
                DrawTool::Shape => {
                    app.creation_origin = Some((cx, cy));
                }
                DrawTool::Text => {
                    app.data.elements.push(DrawElement::Text(Text {
                        content: "New Text".to_string(),
                        x: cx,
                        y: cy,
                        color: (255, 255, 255),
                    }));
                    return Ok(Some(DrawEventAction::Save));
                }
                DrawTool::Erase => {
                    erase_at(cx, cy, app);
                }
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            let (cx, cy) = screen_to_canvas(ev.column, ev.row, app);

            if let Some(idx) = find_text_at(cx, cy, app)
                && let Some(DrawElement::Text(t)) = app.data.elements.get(idx)
            {
                let textarea = TextArea::new(vec![t.content.clone()]);
                app.text_editor = Some((idx, textarea));
                return Ok(None);
            }

            app.last_mouse_pos = Some((ev.column, ev.row));
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            app.last_mouse_pos = Some((ev.column, ev.row));
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            let (cx, cy) = screen_to_canvas(ev.column, ev.row, app);
            match app.active_tool {
                DrawTool::Draw => {
                    if let Some(stroke) = &mut app.current_stroke {
                        stroke.points.push((cx, cy));
                    }
                }
                DrawTool::Erase => {
                    erase_at(cx, cy, app);
                }
                DrawTool::Shape => {
                    if let Some((ox, oy)) = app.creation_origin {
                        app.preview_element =
                            Some(create_shape(ox, oy, cx, cy, app.active_shape_type));
                    }
                }
                DrawTool::Text => {}
            }
        }
        MouseEventKind::Drag(MouseButton::Right) | MouseEventKind::Drag(MouseButton::Middle) => {
            panning(ev.column, ev.row, app);
        }
        MouseEventKind::Up(MouseButton::Left) => {
            let mut changed = false;
            if let Some(stroke) = app.current_stroke.take() {
                app.data.elements.push(DrawElement::Stroke(stroke));
                changed = true;
            }
            if let Some(element) = app.preview_element.take() {
                app.data.elements.push(element);
                changed = true;
            }
            if app.active_tool == DrawTool::Erase {
                changed = true;
            }
            app.creation_origin = None;
            if changed {
                return Ok(Some(DrawEventAction::Save));
            }
        }
        MouseEventKind::Up(_) => {
            app.last_mouse_pos = None;
        }
        MouseEventKind::ScrollUp => {
            app.viewport.zoom *= 1.1;
        }
        MouseEventKind::ScrollDown => {
            app.viewport.zoom /= 1.1;
        }
        _ => {}
    }

    Ok(None)
}

fn create_shape(ox: f64, oy: f64, cx: f64, cy: f64, st: DrawShapeType) -> DrawElement {
    let color = (255, 255, 255);
    match st {
        DrawShapeType::Rect => DrawElement::Shape(Shape::Rect {
            x: ox.min(cx),
            y: oy.min(cy),
            width: (ox - cx).abs(),
            height: (oy - cy).abs(),
            color,
        }),
        DrawShapeType::Ellipse => DrawElement::Shape(Shape::Ellipse {
            x: ox.min(cx),
            y: oy.min(cy),
            width: (ox - cx).abs(),
            height: (oy - cy).abs(),
            color,
        }),
        DrawShapeType::Diamond => DrawElement::Shape(Shape::Diamond {
            x: ox.min(cx),
            y: oy.min(cy),
            width: (ox - cx).abs(),
            height: (oy - cy).abs(),
            color,
        }),
        DrawShapeType::Line => DrawElement::Shape(Shape::Line {
            x1: ox,
            y1: oy,
            x2: cx,
            y2: cy,
            color,
        }),
        DrawShapeType::Arrow => DrawElement::Shape(Shape::Arrow {
            x1: ox,
            y1: oy,
            x2: cx,
            y2: cy,
            color,
        }),
    }
}

fn find_text_at(cx: f64, cy: f64, app: &DrawAppState) -> Option<usize> {
    let threshold = 5.0 / app.viewport.zoom;
    for (i, el) in app.data.elements.iter().enumerate() {
        if let DrawElement::Text(t) = el
            && ((t.x - cx).powi(2) + (t.y - cy).powi(2)).sqrt() < threshold * 2.0
        {
            return Some(i);
        }
    }
    None
}

fn panning(col: u16, row: u16, app: &mut DrawAppState) {
    if let Some((last_col, last_row)) = app.last_mouse_pos {
        let dx = col as f64 - last_col as f64;
        let dy = row as f64 - last_row as f64;

        let x_range = 200.0 / app.viewport.zoom;
        let y_range = 200.0 / app.viewport.zoom;

        let area = app.last_area;
        if area.width > 0 && area.height > 0 {
            app.viewport.x -= (dx / area.width as f64) * x_range;
            app.viewport.y += (dy / area.height as f64) * y_range;
        }

        app.last_mouse_pos = Some((col, row));
    }
}

fn erase_at(cx: f64, cy: f64, app: &mut DrawAppState) {
    let threshold = 5.0 / app.viewport.zoom;
    app.data.elements.retain(|el| match el {
        DrawElement::Stroke(s) => !s
            .points
            .iter()
            .any(|(px, py)| ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() < threshold),
        DrawElement::Shape(s) => match s {
            Shape::Rect {
                x,
                y,
                width,
                height,
                ..
            } => !(cx >= *x && cx <= x + width && cy >= *y && cy <= y + height),
            Shape::Ellipse {
                x,
                y,
                width,
                height,
                ..
            } => {
                let rx = width / 2.0;
                let ry = height / 2.0;
                let cx_center = x + rx;
                let cy_center = y + ry;
                if rx == 0.0 || ry == 0.0 {
                    false
                } else {
                    ((cx - cx_center).powi(2) / rx.powi(2) + (cy - cy_center).powi(2) / ry.powi(2))
                        > 1.0
                }
            }
            Shape::Diamond {
                x,
                y,
                width,
                height,
                ..
            } => !(cx >= *x && cx <= x + width && cy >= *y && cy <= y + height),
            Shape::Line { x1, y1, x2, y2, .. } => {
                let d = ((x2 - x1) * (y1 - cy) - (x1 - cx) * (y2 - y1)).abs()
                    / ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
                d >= threshold
            }
            Shape::Arrow { x1, y1, x2, y2, .. } => {
                let d = ((x2 - x1) * (y1 - cy) - (x1 - cx) * (y2 - y1)).abs()
                    / ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
                d >= threshold
            }
        },
        DrawElement::Text(t) => ((t.x - cx).powi(2) + (t.y - cy).powi(2)).sqrt() >= threshold * 2.0,
    });
}

fn screen_to_canvas(col: u16, row: u16, app: &DrawAppState) -> (f64, f64) {
    let area = app.last_area;
    if area.width == 0 || area.height == 0 {
        return (0.0, 0.0);
    }

    let x_range = 200.0 / app.viewport.zoom;
    let y_range = 200.0 / app.viewport.zoom;

    let rel_x = (col as f64 - area.x as f64) / area.width as f64;
    let rel_y = (row as f64 - area.y as f64) / area.height as f64;

    let cx = app.viewport.x + (rel_x - 0.5) * x_range;
    let cy = app.viewport.y + (0.5 - rel_y) * y_range;

    (cx, cy)
}
