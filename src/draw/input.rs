use crate::draw::app::{DrawAppState, DrawEventAction, DrawInteraction};
use crate::draw::state::{DrawElement, DrawItem, DrawShapeType, DrawTool, Shape, Stroke, Text};
use crate::keybinds::{DrawAction, Keybinds};
use crate::text_edit::apply_text_shortcuts;
use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Margin};

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
                let changed = app.data.item(&target).is_some_and(|item| {
                    matches!(&item.element, DrawElement::Text(text) if text.content != new_content)
                });
                if changed {
                    let previous = app.data.clone();
                    if let Some(item) = app.data.item_mut(&target)
                        && let DrawElement::Text(text) = &mut item.element
                    {
                        text.content = new_content;
                    }
                    app.commit_data_change(previous)?;
                }
                app.text_editor = None;
                return Ok(None);
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
                app.set_active_tool(DrawTool::Shape);
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
                    app.set_active_tool(DrawTool::Draw);
                    return Ok(None);
                }
                DrawAction::ToggleShapeSelector => {
                    app.show_shape_selector = !app.show_shape_selector;
                    return Ok(None);
                }
                DrawAction::SelectTextTool => {
                    app.set_active_tool(DrawTool::Text);
                    return Ok(None);
                }
                DrawAction::SelectEraseTool => {
                    app.set_active_tool(DrawTool::Erase);
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
    app.mouse_pos = Some((ev.column, ev.row));
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
                    if let Some(&shape) = shapes.get(row_rel) {
                        app.active_shape_type = shape;
                        app.set_active_tool(DrawTool::Shape);
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
        MouseEventKind::Moved => {
            if app.active_tool == DrawTool::Cursor && app.interaction.is_none() && !app.is_panning {
                let point = screen_to_canvas(ev.column, ev.row, app);
                app.hovered = app.topmost_hit(point);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let icon_mode = config.ui.icon_mode;
            let header_y = area.y.saturating_sub(1);
            if ev.row == header_y {
                let tabs_arr = crate::draw::render::draw_tool_tabs(icon_mode);
                let tabs = crate::ui::tab_vec_from_array(&tabs_arr);
                let region = crate::ui::title_bar_tabs_region(area, "Draw");
                if let Some(index) = crate::ui::hit_test_tabs(
                    &tabs, area.x, area.width, region.x, ev.column, false, icon_mode,
                ) {
                    let tool = crate::draw::render::DRAW_TAB_TOOLS[index];
                    if tool == DrawTool::Shape {
                        app.clear_transient_interaction();
                        app.show_shape_selector = true;
                    } else {
                        app.set_active_tool(tool);
                    }
                }
                return Ok(None);
            }

            let point = screen_to_canvas(ev.column, ev.row, app);
            match app.active_tool {
                DrawTool::Cursor => cursor_left_down(ev, point, app),
                DrawTool::Draw => {
                    app.current_stroke = Some(Stroke {
                        points: vec![point],
                        color: (255, 255, 255),
                    });
                }
                DrawTool::Shape => {
                    app.creation_origin = Some(point);
                }
                DrawTool::Text => {
                    let previous = app.data.clone();
                    app.data
                        .elements
                        .push(DrawItem::new(DrawElement::Text(Text {
                            content: "New Text".to_string(),
                            x: point.0,
                            y: point.1,
                            color: (255, 255, 255),
                        })));
                    app.commit_data_change(previous)?;
                }
                DrawTool::Erase => {
                    app.erase_start_data = Some(app.data.clone());
                    erase_at(point.0, point.1, app);
                }
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            let point = screen_to_canvas(ev.column, ev.row, app);
            app.right_mouse_target = app.topmost_hit(point);
            app.right_mouse_screen = Some((ev.column, ev.row));
            app.right_mouse.on_down(point.0, point.1);
            app.last_mouse_pos = Some((ev.column, ev.row));
        }
        MouseEventKind::Drag(MouseButton::Right) => {
            if let Some((start_x, start_y)) = app.right_mouse_screen
                && app
                    .right_mouse
                    .is_dragging_screen(ev.column, ev.row, start_x, start_y)
            {
                let point = screen_to_canvas(ev.column, ev.row, app);
                app.right_mouse.on_drag(point.0, point.1);
                panning(ev.column, ev.row, app);
            }
        }
        MouseEventKind::Up(MouseButton::Right) => {
            let dragged = app.right_mouse_screen.is_some_and(|(start_x, start_y)| {
                app.right_mouse
                    .is_dragging_screen(ev.column, ev.row, start_x, start_y)
            });
            if dragged {
                app.is_panning = false;
                app.last_mouse_pos = None;
            } else if let Some(id) = app.right_mouse_target.clone() {
                app.selection.select_only(id);
                app.hovered = None;
            } else {
                app.selection.clear();
                app.hovered = None;
            }
            app.right_mouse.clear();
            app.right_mouse_screen = None;
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            app.last_mouse_pos = Some((ev.column, ev.row));
        }
        MouseEventKind::Drag(MouseButton::Middle) => {
            panning(ev.column, ev.row, app);
        }
        MouseEventKind::Up(MouseButton::Middle) => {
            app.is_panning = false;
            app.last_mouse_pos = None;
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            let point = screen_to_canvas(ev.column, ev.row, app);
            match app.active_tool {
                DrawTool::Cursor => cursor_left_drag(point, app),
                DrawTool::Text => {}
                DrawTool::Draw => {
                    if let Some(stroke) = &mut app.current_stroke {
                        stroke.points.push(point);
                    }
                }
                DrawTool::Erase => {
                    erase_at(point.0, point.1, app);
                }
                DrawTool::Shape => {
                    if let Some(origin) = app.creation_origin {
                        app.preview_element = Some(create_shape(
                            origin.0,
                            origin.1,
                            point.0,
                            point.1,
                            app.active_shape_type,
                        ));
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if app.active_tool == DrawTool::Cursor {
                finish_cursor_move(app)?;
                return Ok(None);
            }

            let mut previous = None;
            if let Some(mut stroke) = app.current_stroke.take() {
                stroke.points = crate::draw::render::smooth_points(&stroke.points);
                previous = Some(app.data.clone());
                app.data
                    .elements
                    .push(DrawItem::new(DrawElement::Stroke(stroke)));
            }
            if let Some(element) = app.preview_element.take() {
                if previous.is_none() {
                    previous = Some(app.data.clone());
                }
                app.data.elements.push(DrawItem::new(element));
            }
            if app.active_tool == DrawTool::Erase {
                previous = app.erase_start_data.take();
            }
            app.creation_origin = None;
            if let Some(previous) = previous {
                app.commit_data_change(previous)?;
            }
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

fn cursor_left_down(mouse: MouseEvent, point: (f64, f64), app: &mut DrawAppState) {
    let hit = app.topmost_hit(point);
    let double_click = app.last_click.is_some_and(|(column, row, at)| {
        column == mouse.column && row == mouse.row && at.elapsed().as_millis() < 500
    });

    if let Some(id) = hit {
        let is_text = app
            .data
            .item(&id)
            .is_some_and(|item| matches!(&item.element, DrawElement::Text(_)));
        if double_click && is_text {
            app.selection.select_only(id.clone());
            app.hovered = None;
            app.begin_text_editor(id);
            app.last_click = None;
            return;
        }

        let translation = app.data.item(&id).map_or((0.0, 0.0), |item| {
            (item.transform.translate_x, item.transform.translate_y)
        });
        app.selection.select_only(id.clone());
        app.hovered = None;
        app.interaction = Some(DrawInteraction::Move {
            id,
            start_world: point,
            original_translation: translation,
            preview_translation: translation,
        });
    } else {
        app.selection.clear();
        app.hovered = None;
        app.interaction = None;
    }
    app.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
}

fn cursor_left_drag(point: (f64, f64), app: &mut DrawAppState) {
    let Some(DrawInteraction::Move {
        start_world,
        original_translation,
        preview_translation,
        ..
    }) = &mut app.interaction
    else {
        return;
    };
    preview_translation.0 = original_translation.0 + point.0 - start_world.0;
    preview_translation.1 = original_translation.1 + point.1 - start_world.1;
}

fn finish_cursor_move(app: &mut DrawAppState) -> anyhow::Result<()> {
    let Some(DrawInteraction::Move {
        id,
        original_translation,
        preview_translation,
        ..
    }) = app.interaction.take()
    else {
        return Ok(());
    };
    if original_translation == preview_translation {
        return Ok(());
    }

    let previous = app.data.clone();
    if let Some(item) = app.data.item_mut(&id) {
        item.transform.translate_x = preview_translation.0;
        item.transform.translate_y = preview_translation.1;
        app.commit_data_change(previous)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use ratatui::layout::Rect;

    fn test_state() -> (tempfile::TempDir, DrawAppState) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let storage = crate::storage::Storage {
            data_dir: root.join("data"),
            config_dir: root.join("config"),
            notes_dir: root.join("notes"),
            templates_dir: root.join("templates"),
            key: [0; 32],
            skip_dir_patterns: Vec::new(),
        };
        std::fs::create_dir_all(&storage.notes_dir).unwrap();
        (
            temp,
            DrawAppState::new(
                storage,
                Some("cursor.draw".to_string()),
                crate::app_theme::AppThemeColors::default(),
                Keybinds::default(),
                crate::keybinds::KeyMatcher::new(),
            ),
        )
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    #[test]
    fn cursor_drag_commits_one_translated_item() {
        let (_temp, mut state) = test_state();
        state.last_area = Rect::new(0, 0, 100, 100);
        let item = DrawItem::new(DrawElement::Shape(Shape::Rect {
            x: -2.0,
            y: -2.0,
            width: 4.0,
            height: 4.0,
            color: (255, 255, 255),
        }));
        let id = item.id.clone();
        state.data.elements.push(item);
        let keybinds = Keybinds::default();
        let config = crate::config::ClinConfig::default();

        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 50, 50),
            &mut state,
            &keybinds,
            &config,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Drag(MouseButton::Left), 60, 60),
            &mut state,
            &keybinds,
            &config,
        )
        .unwrap();
        assert_eq!(state.data.item(&id).unwrap().transform.translate_x, 0.0);
        let Some(DrawInteraction::Move {
            id: preview_id,
            start_world,
            original_translation,
            preview_translation,
        }) = &state.interaction
        else {
            panic!("cursor drag must create move preview");
        };
        assert_eq!(preview_id, &id);
        assert!((start_world.0 - 1.0).abs() < 1e-9);
        assert!((start_world.1 + 1.0).abs() < 1e-9);
        assert_eq!(*original_translation, (0.0, 0.0));
        assert!((preview_translation.0 - 20.0).abs() < 1e-9);
        assert!((preview_translation.1 + 20.0).abs() < 1e-9);

        handle_event(
            mouse(MouseEventKind::Up(MouseButton::Left), 60, 60),
            &mut state,
            &keybinds,
            &config,
        )
        .unwrap();
        let item = state.data.item(&id).unwrap();
        assert!((item.transform.translate_x - 20.0).abs() < 1e-9);
        assert!((item.transform.translate_y + 20.0).abs() < 1e-9);
        assert_eq!(state.undo_stack.len(), 1);
        assert!(state.interaction.is_none());
    }

    #[test]
    fn cursor_double_click_opens_text_editor() {
        let (_temp, mut state) = test_state();
        state.last_area = Rect::new(0, 0, 100, 100);
        let item = DrawItem::new(DrawElement::Text(Text {
            content: "text".to_string(),
            x: 0.0,
            y: 0.0,
            color: (255, 255, 255),
        }));
        let id = item.id.clone();
        state.data.elements.push(item);
        let keybinds = Keybinds::default();
        let config = crate::config::ClinConfig::default();

        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 50, 49),
            &mut state,
            &keybinds,
            &config,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Up(MouseButton::Left), 50, 49),
            &mut state,
            &keybinds,
            &config,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 50, 49),
            &mut state,
            &keybinds,
            &config,
        )
        .unwrap();

        assert_eq!(
            state.text_editor.as_ref().map(|(target, _)| target),
            Some(&id)
        );
        assert!(state.interaction.is_none());
    }
}
