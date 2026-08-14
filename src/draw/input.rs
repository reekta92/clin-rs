use crate::draw::app::{DrawAppState, DrawEventAction};
use crate::draw::state::{
    DrawElement, DrawItem, DrawItemId, DrawShapeType, DrawTool, Shape, Stroke, Text,
};
use crate::keybinds::{DrawAction, Keybinds};
use crate::text_edit::apply_text_shortcuts;
use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Margin};
use ratatui_textarea::TextArea;

pub fn handle_event(
    ev: Event,
    app: &mut DrawAppState,
    keybinds: &Keybinds,
    config: &crate::config::ClinConfig,
) -> anyhow::Result<Option<DrawEventAction>> {
    if app.text_editor.is_some()
        && let Event::Mouse(mouse) = ev
    {
        return handle_text_editor_mouse(mouse, app);
    }

    if let Some((id, textarea)) = &mut app.text_editor {
        app.seq_matcher.clear();
        match ev {
            Event::Key(k) if keybinds.matches_draw(DrawAction::TextEditorCancel, &k) => {
                app.text_editor = None;
                return Ok(None);
            }
            Event::Key(k) if keybinds.matches_draw(DrawAction::TextEditorConfirm, &k) => {
                let new_content = textarea.lines()[0].clone();
                let target = id.clone();
                if let Some(item) = app.data.elements.iter_mut().find(|item| item.id == target)
                    && let DrawElement::Text(text) = &mut item.element
                {
                    text.content = new_content;
                }
                app.text_editor = None;
                return Ok(Some(DrawEventAction::Save));
            }
            _ => {
                if let Event::Key(k) = ev
                    && apply_text_shortcuts(keybinds, textarea, k)
                {
                    return Ok(None);
                }
                textarea.input(ev);
                return Ok(None);
            }
        }
    }

    if app.show_shape_selector {
        app.seq_matcher.clear();
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

    if let Event::Key(k) = ev {
        if crate::events::is_universal_quit_key(&k) {
            return Ok(Some(DrawEventAction::Quit));
        }
        let seq = config.sequences_enabled();
        let counts = config.counts_enabled();
        match keybinds.resolve_draw(&mut app.seq_matcher, k, seq, counts) {
            crate::keybinds::MatchOutcome::Matched(action, _count) => match action {
                DrawAction::Quit => {
                    return Ok(Some(DrawEventAction::Quit));
                }
                DrawAction::SelectDrawTool => {
                    app.active_tool = DrawTool::Draw;
                    return Ok(None);
                }
                DrawAction::ToggleShapeSelector => {
                    app.show_shape_selector = !app.show_shape_selector;
                    return Ok(None);
                }
                DrawAction::SelectTextTool => {
                    app.active_tool = DrawTool::Text;
                    return Ok(None);
                }
                DrawAction::SelectEraseTool => {
                    app.active_tool = DrawTool::Erase;
                    return Ok(None);
                }
                DrawAction::Help => {
                    return Ok(Some(DrawEventAction::OpenHelp));
                }
                DrawAction::ToggleGrid => {
                    app.show_grid = !app.show_grid;
                    return Ok(None);
                }
                _ => {
                    if keybinds.matches_draw(DrawAction::Quit, &k) {
                        return Ok(Some(DrawEventAction::Quit));
                    }
                }
            },
            crate::keybinds::MatchOutcome::Pending => return Ok(None),
            crate::keybinds::MatchOutcome::NoMatch => {}
        }
    }

    match ev {
        Event::Mouse(mouse_event) => handle_mouse(mouse_event, app, config),
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

fn handle_text_editor_mouse(
    mouse: MouseEvent,
    app: &mut DrawAppState,
) -> anyhow::Result<Option<DrawEventAction>> {
    let Some((_, textarea)) = &mut app.text_editor else {
        return Ok(None);
    };
    let Some(textarea_area) = app.text_editor_rect else {
        return Ok(None);
    };

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left)
            if crate::events::contains_cell(textarea_area, mouse.column, mouse.row) =>
        {
            let (scroll_row, scroll_col) = crate::ui::get_textarea_scroll(textarea);
            crate::events::move_textarea_cursor_to_mouse(
                textarea,
                textarea_area,
                mouse.column,
                mouse.row,
                scroll_row,
                scroll_col,
            );
            app.mouse_selection.begin(textarea);
        }
        MouseEventKind::Drag(MouseButton::Left) if app.mouse_selection.active => {
            app.mouse_selection.mark_drag();
            let (scroll_row, scroll_col) = crate::ui::get_textarea_scroll(textarea);
            crate::events::move_textarea_cursor_to_mouse(
                textarea,
                textarea_area,
                mouse.column,
                mouse.row,
                scroll_row,
                scroll_col,
            );
        }
        MouseEventKind::Up(MouseButton::Left) if app.mouse_selection.active => {
            app.mouse_selection.finish(textarea);
        }
        _ => {}
    }

    Ok(None)
}

fn handle_mouse(
    ev: MouseEvent,
    app: &mut DrawAppState,
    config: &crate::config::ClinConfig,
) -> anyhow::Result<Option<DrawEventAction>> {
    let area = app.last_area;

    if app.show_shape_selector {
        let popup_area = crate::ui::centered_rect(crate::ui::PopupSize::Small, area);
        let content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(popup_area)[0];

        if ev.kind == MouseEventKind::Down(MouseButton::Left) {
            if crate::events::contains_cell(content, ev.column, ev.row) {
                let inner = content.inner(Margin {
                    vertical: 1,
                    horizontal: 1,
                });
                if crate::events::contains_cell(inner, ev.column, ev.row) {
                    let row_rel = (ev.row - inner.y) as usize;
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
                }
            } else {
                app.show_shape_selector = false;
            }
        }
        return Ok(None);
    }

    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let icon_mode = config.ui.icon_mode;
            let header_y = area.y.saturating_sub(1);
            if ev.row == header_y {
                let tabs_arr = crate::draw::render::draw_tool_tabs(icon_mode);
                let tabs = crate::ui::tab_vec_from_array(&tabs_arr);
                let region = crate::ui::title_bar_tabs_region(area, "Draw");
                if let Some(i) = crate::ui::hit_test_tabs(
                    &tabs, area.x, area.width, region.x, ev.column, false, icon_mode,
                ) {
                    if crate::draw::render::DRAW_TAB_TOOLS[i] == DrawTool::Shape {
                        app.show_shape_selector = true;
                    } else {
                        app.active_tool = crate::draw::render::DRAW_TAB_TOOLS[i];
                    }
                }
                return Ok(None);
            }

            let (cx, cy) = screen_to_canvas(ev.column, ev.row, app);

            match app.active_tool {
                DrawTool::Cursor => {}
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
                    app.data
                        .elements
                        .push(DrawItem::new(DrawElement::Text(Text {
                            content: "New Text".to_string(),
                            x: cx,
                            y: cy,
                            color: (255, 255, 255),
                        })));
                    return Ok(Some(DrawEventAction::Save));
                }
                DrawTool::Erase => {
                    erase_at(cx, cy, app);
                }
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            let (cx, cy) = screen_to_canvas(ev.column, ev.row, app);

            if let Some(id) = find_text_at(cx, cy, app)
                && let Some(item) = app.data.elements.iter().find(|item| item.id == id)
                && let DrawElement::Text(text) = &item.element
            {
                let textarea = TextArea::new(vec![text.content.clone()]);
                app.text_editor = Some((id, textarea));
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
                DrawTool::Cursor | DrawTool::Text => {}
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
            }
        }
        MouseEventKind::Drag(MouseButton::Right) | MouseEventKind::Drag(MouseButton::Middle) => {
            panning(ev.column, ev.row, app);
        }
        MouseEventKind::Up(MouseButton::Left) => {
            let mut changed = false;
            if let Some(mut stroke) = app.current_stroke.take() {
                stroke.points = crate::draw::render::smooth_points(&stroke.points);
                app.data
                    .elements
                    .push(DrawItem::new(DrawElement::Stroke(stroke)));
                changed = true;
            }
            if let Some(element) = app.preview_element.take() {
                app.data.elements.push(DrawItem::new(element));
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
            app.is_panning = false;
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

fn find_text_at(cx: f64, cy: f64, app: &DrawAppState) -> Option<DrawItemId> {
    app.data.elements.iter().rev().find_map(|item| {
        (matches!(&item.element, DrawElement::Text(_))
            && crate::draw::geometry::hit_test_item(item, (cx, cy), 5.0, &app.viewport))
        .then(|| item.id.clone())
    })
}

fn erase_at(cx: f64, cy: f64, app: &mut DrawAppState) {
    app.data
        .elements
        .retain(|item| !crate::draw::geometry::hit_test_item(item, (cx, cy), 5.0, &app.viewport));
}

fn panning(x: u16, y: u16, app: &mut DrawAppState) {
    app.is_panning = true;
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
