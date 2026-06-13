use crate::draw::app::{DrawAppState, DrawEventAction};
use crate::draw::state::{DrawElement, DrawShapeType, DrawTool, Shape, Stroke, Text};
use crate::keybinds::{DrawAction, Keybinds};
use crate::text_edit::apply_text_shortcuts;
use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use ratatui_textarea::TextArea;

pub fn handle_event(
    ev: Event,
    app: &mut DrawAppState,
    keybinds: &Keybinds,
) -> anyhow::Result<Option<DrawEventAction>> {
    if let Some((idx, textarea)) = &mut app.text_editor {
        match ev {
            Event::Key(k) if keybinds.matches_draw(DrawAction::TextEditorCancel, &k) => {
                app.text_editor = None;
                return Ok(None);
            }
            Event::Key(k) if keybinds.matches_draw(DrawAction::TextEditorConfirm, &k) => {
                let new_content = textarea.lines()[0].clone();
                if let Some(DrawElement::Text(t)) = app.data.elements.get_mut(*idx) {
                    t.content = new_content;
                }
                app.text_editor = None;
                return Ok(Some(DrawEventAction::Save));
            }
            _ => {
                if let Event::Key(k) = ev {
                    if apply_text_shortcuts(keybinds, textarea, k) {
                        return Ok(None);
                    }
                }
                textarea.input(ev);
                return Ok(None);
            }
        }
    }

    if app.show_shape_selector {
        match ev {
            Event::Key(k) if keybinds.matches_draw(DrawAction::ShapeSelectorCancel, &k) => {
                app.show_shape_selector = false;
                return Ok(None);
            }
            Event::Key(k) if keybinds.matches_draw(DrawAction::ShapeSelectorConfirm, &k) => {
                app.show_shape_selector = false;
                app.active_tool = DrawTool::Shape;
                return Ok(None);
            }
            Event::Key(k) if keybinds.matches_draw(DrawAction::ShapeSelectorUp, &k) => {
                cycle_shape_type(app, -1);
                return Ok(None);
            }
            Event::Key(k) if keybinds.matches_draw(DrawAction::ShapeSelectorDown, &k) => {
                cycle_shape_type(app, 1);
                return Ok(None);
            }
            _ => {}
        }
    }

    match ev {
        Event::Key(k) if keybinds.matches_draw(DrawAction::Quit, &k) => {
            Ok(Some(DrawEventAction::Quit))
        }
        Event::Key(k) if keybinds.matches_draw(DrawAction::SelectDrawTool, &k) => {
            app.active_tool = DrawTool::Draw;
            Ok(None)
        }
        Event::Key(k) if keybinds.matches_draw(DrawAction::ToggleShapeSelector, &k) => {
            app.show_shape_selector = !app.show_shape_selector;
            Ok(None)
        }
        Event::Key(k) if keybinds.matches_draw(DrawAction::SelectTextTool, &k) => {
            app.active_tool = DrawTool::Text;
            Ok(None)
        }
        Event::Key(k) if keybinds.matches_draw(DrawAction::SelectEraseTool, &k) => {
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
        if let DrawElement::Text(t) = el {
            if (t.x - cx).abs() < threshold && (t.y - cy).abs() < threshold {
                return Some(i);
            }
        }
    }
    None
}

fn erase_at(cx: f64, cy: f64, app: &mut DrawAppState) {
    let threshold = 5.0 / app.viewport.zoom;
    app.data.elements.retain(|el| match el {
        DrawElement::Stroke(s) => !s.points.iter().any(|(px, py)| {
            ((*px as f64) - cx).powi(2) + ((*py as f64) - cy).powi(2) < threshold.powi(2)
        }),
        DrawElement::Shape(s) => match s {
            Shape::Rect { x, y, width, height, .. }
            | Shape::Ellipse { x, y, width, height, .. }
            | Shape::Diamond { x, y, width, height, .. } => {
                cx < *x || cx > *x + *width || cy < *y || cy > *y + *height
            }
            Shape::Line { x1, y1, x2, y2, .. } | Shape::Arrow { x1, y1, x2, y2, .. } => {
                let d = line_dist(*x1, *y1, *x2, *y2, cx, cy);
                d > threshold
            }
        },
        DrawElement::Text(t) => (t.x - cx).abs() > threshold || (t.y - cy).abs() > threshold,
    });
}

fn line_dist(x1: f64, y1: f64, x2: f64, y2: f64, px: f64, py: f64) -> f64 {
    let l2 = (x2 - x1).powi(2) + (y2 - y1).powi(2);
    if l2 == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = ((px - x1) * (x2 - x1) + (py - y1) * (y2 - y1)) / l2;
    let t = t.clamp(0.0, 1.0);
    ((px - (x1 + t * (x2 - x1))).powi(2) + (py - (y1 + t * (y2 - y1))).powi(2)).sqrt()
}

fn panning(x: u16, y: u16, app: &mut DrawAppState) {
    if let Some((lx, ly)) = app.last_mouse_pos {
        let area = app.last_area;
        if area.width > 0 && area.height > 0 {
            let dx = (lx as f64 - x as f64) * 200.0 / (area.width as f64 * app.viewport.zoom);
            let dy = (y as f64 - ly as f64) * 200.0 / (area.height as f64 * app.viewport.zoom);
            app.viewport.x += dx;
            app.viewport.y += dy;
        }
        app.last_mouse_pos = Some((x, y));
    }
}

fn screen_to_canvas(sx: u16, sy: u16, app: &DrawAppState) -> (f64, f64) {
    let area = app.last_area;
    if area.width == 0 || area.height == 0 {
        return (0.0, 0.0);
    }
    let col_frac = (sx as f64 - area.x as f64 + 0.5) / area.width as f64;
    let row_frac = (sy as f64 - area.y as f64 + 0.5) / area.height as f64;

    let cx = app.viewport.x + (col_frac * 2.0 - 1.0) * 100.0 / app.viewport.zoom;
    let cy = app.viewport.y + (1.0 - row_frac * 2.0) * 100.0 / app.viewport.zoom;
    (cx, cy)
}
