use crate::draw::app::{
    DrawAppState, DrawEventAction, DrawInteraction, DrawMenuItem, DrawMenuKind, DrawMenuTarget,
    draw_menu_items,
};
use crate::draw::state::{
    DrawClipboard, DrawElement, DrawItem, DrawShapeType, DrawTool, Shape, Stroke, Text,
};
use crate::keybinds::{DrawAction, Keybinds};
use crate::text_edit::apply_text_shortcuts;
use crossterm::event::{Event, KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Margin};

pub fn handle_event(
    ev: Event,
    app: &mut DrawAppState,
    keybinds: &Keybinds,
    config: &crate::config::ClinConfig,
    clipboard: &mut Option<DrawClipboard>,
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

    if app.context_menu.is_some()
        && let Event::Key(key) = ev
    {
        return handle_context_menu_key(key, app, keybinds, clipboard);
    }

    if let Event::Key(k) = ev {
        if keybinds.matches_draw(DrawAction::MenuClose, &k) {
            if app.interaction.is_some() {
                app.interaction = None;
                return Ok(None);
            }
            if !app.selection.is_empty() {
                app.selection.clear();
                app.hovered = None;
                return Ok(None);
            }
        }
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
                DrawAction::SelectCursorTool => {
                    app.set_active_tool(DrawTool::Cursor);
                    return Ok(None);
                }
                DrawAction::SelectDrawTool => {
                    app.set_active_tool(DrawTool::Draw);
                    return Ok(None);
                }
                DrawAction::ToggleShapeSelector => {
                    if app.show_shape_selector {
                        app.show_shape_selector = false;
                    } else {
                        app.clear_transient_interaction();
                        app.show_shape_selector = true;
                    }
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
                DrawAction::Copy => {
                    copy_selected(app, clipboard);
                    return Ok(None);
                }
                DrawAction::Paste => {
                    begin_paste(app, clipboard.as_ref(), None);
                    return Ok(None);
                }
                DrawAction::Undo => {
                    app.undo()?;
                    return Ok(None);
                }
                DrawAction::Redo => {
                    app.redo()?;
                    return Ok(None);
                }
                DrawAction::Help => {
                    return Ok(Some(DrawEventAction::OpenHelp));
                }
                DrawAction::ToggleGrid => {
                    app.grid.toggle();
                    return Ok(None);
                }
                DrawAction::MenuClose
                | DrawAction::MenuUp
                | DrawAction::MenuDown
                | DrawAction::MenuSelect => {}
                DrawAction::ShapeSelectorUp
                | DrawAction::ShapeSelectorDown
                | DrawAction::ShapeSelectorConfirm
                | DrawAction::ShapeSelectorCancel
                | DrawAction::TextEditorConfirm
                | DrawAction::TextEditorCancel => {}
            },
            crate::keybinds::MatchOutcome::Pending => return Ok(None),
            crate::keybinds::MatchOutcome::NoMatch => {}
        }
    }

    match ev {
        Event::Mouse(mouse_event) => handle_mouse(mouse_event, app, config, clipboard),
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
    clipboard: &mut Option<DrawClipboard>,
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

    if app.context_menu.is_some() {
        return handle_context_menu_mouse(ev, app, clipboard);
    }
    if matches!(
        &app.interaction,
        Some(DrawInteraction::Rotate { .. })
            | Some(DrawInteraction::Scale { .. })
            | Some(DrawInteraction::Paste { .. })
    ) {
        return handle_transform_or_paste_mouse(ev, app);
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
            } else {
                let point = screen_to_canvas(ev.column, ev.row, app);
                let target = app.right_mouse_target.take().map_or(
                    DrawMenuTarget::Empty {
                        x: point.0,
                        y: point.1,
                    },
                    |id| {
                        if app
                            .data
                            .item(&id)
                            .is_some_and(|item| matches!(&item.element, DrawElement::Text(_)))
                        {
                            app.selection.select_only(id.clone());
                            DrawMenuTarget::Text(id)
                        } else {
                            app.selection.select_only(id.clone());
                            DrawMenuTarget::NonText(id)
                        }
                    },
                );
                if matches!(target, DrawMenuTarget::Empty { .. }) {
                    app.selection.clear();
                }
                app.hovered = None;
                app.open_context_menu(ev.column, ev.row, target, clipboard.is_some());
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
                DrawTool::Cursor => cursor_left_drag(ev, point, app),
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
                app.is_panning = false;
                app.last_mouse_pos = None;
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

fn handle_context_menu_key(
    key: crossterm::event::KeyEvent,
    app: &mut DrawAppState,
    keybinds: &Keybinds,
    clipboard: &mut Option<DrawClipboard>,
) -> anyhow::Result<Option<DrawEventAction>> {
    let mut menu_action = None;
    let mut close_menu = false;

    if let Some(menu) = &mut app.context_menu {
        app.seq_matcher.clear();
        if keybinds.matches_draw(DrawAction::MenuClose, &key) {
            close_menu = true;
        } else if keybinds.matches_draw(DrawAction::MenuUp, &key) {
            menu.move_up();
        } else if keybinds.matches_draw(DrawAction::MenuDown, &key) {
            menu.move_down();
        } else if keybinds.matches_draw(DrawAction::MenuSelect, &key) {
            menu_action = Some((menu.selected, menu.x, menu.y));
            close_menu = true;
        } else if let KeyCode::Char(shortcut) = key.code
            && let Some(index) = menu.find_shortcut(shortcut)
        {
            menu_action = Some((index, menu.x, menu.y));
            close_menu = true;
        }
    }

    if let Some((index, menu_x, menu_y)) = menu_action {
        let target = app.menu_target.clone();
        let kind = app.menu_kind.take();
        app.context_menu = None;
        if let (Some(target), Some(kind)) = (target, kind) {
            execute_menu_item(app, clipboard, target, kind, index, menu_x, menu_y)?;
        } else {
            app.menu_target = None;
        }
    } else if close_menu {
        app.context_menu = None;
        app.menu_target = None;
        app.menu_kind = None;
    }

    Ok(None)
}

fn handle_context_menu_mouse(
    ev: MouseEvent,
    app: &mut DrawAppState,
    clipboard: &mut Option<DrawClipboard>,
) -> anyhow::Result<Option<DrawEventAction>> {
    if ev.kind != MouseEventKind::Down(MouseButton::Left) {
        return Ok(None);
    }

    let menu_action = app.context_menu.as_ref().and_then(|menu| {
        menu.row_at(menu.rect(app.last_area), ev.column, ev.row)
            .map(|index| (index, menu.x, menu.y))
    });
    let target = app.menu_target.clone();
    let kind = app.menu_kind.take();
    app.context_menu = None;

    if let (Some((index, menu_x, menu_y)), Some(target), Some(kind)) = (menu_action, target, kind) {
        execute_menu_item(app, clipboard, target, kind, index, menu_x, menu_y)?;
    } else {
        app.menu_target = None;
    }
    Ok(None)
}

fn execute_menu_item(
    app: &mut DrawAppState,
    clipboard: &mut Option<DrawClipboard>,
    target: DrawMenuTarget,
    kind: DrawMenuKind,
    index: usize,
    menu_x: u16,
    menu_y: u16,
) -> anyhow::Result<()> {
    match kind {
        DrawMenuKind::Actions => {
            let Some(&item) = draw_menu_items(&target, clipboard.is_some()).get(index) else {
                app.menu_target = None;
                return Ok(());
            };
            match item {
                DrawMenuItem::Rotate => {
                    if let Some(id) = target.item_id() {
                        begin_rotate(app, id.clone());
                    }
                    app.menu_target = None;
                }
                DrawMenuItem::Scale => {
                    if let Some(id) = target.item_id() {
                        begin_scale(app, id.clone());
                    }
                    app.menu_target = None;
                }
                DrawMenuItem::Color => {
                    app.menu_target = Some(target);
                    app.open_color_menu(menu_x, menu_y);
                }
                DrawMenuItem::Copy => {
                    if let Some(id) = target.item_id() {
                        copy_item(app, clipboard, id);
                    }
                    app.menu_target = None;
                }
                DrawMenuItem::Erase => {
                    if let Some(id) = target.item_id() {
                        erase_item(app, id)?;
                    }
                    app.menu_target = None;
                }
                DrawMenuItem::EditText => {
                    if let Some(id) = target.item_id() {
                        app.begin_text_editor(id.clone());
                    }
                    app.menu_target = None;
                }
                DrawMenuItem::Paste => {
                    let anchor = match target {
                        DrawMenuTarget::Empty { x, y } => Some((x, y)),
                        DrawMenuTarget::NonText(_) | DrawMenuTarget::Text(_) => None,
                    };
                    begin_paste(app, clipboard.as_ref(), anchor);
                    app.menu_target = None;
                }
            }
        }
        DrawMenuKind::Color => {
            if let Some(id) = target.item_id()
                && let Some((_, _, ratatui::style::Color::Rgb(red, green, blue))) =
                    crate::pinstar::COLOR_PICKER_PALETTE.get(index)
            {
                set_item_color(app, id, (*red, *green, *blue))?;
            }
            app.menu_target = None;
        }
    }
    Ok(())
}

fn copy_selected(app: &DrawAppState, clipboard: &mut Option<DrawClipboard>) {
    if let Some(id) = &app.selection.primary {
        copy_item(app, clipboard, id);
    }
}

fn copy_item(
    app: &DrawAppState,
    clipboard: &mut Option<DrawClipboard>,
    id: &crate::draw::state::DrawItemId,
) {
    if let Some(item) = app.data.item(id) {
        *clipboard = Some(DrawClipboard::from_item(item));
    }
}

fn begin_paste(
    app: &mut DrawAppState,
    clipboard: Option<&DrawClipboard>,
    anchor: Option<(f64, f64)>,
) {
    let Some(clipboard) = clipboard else {
        return;
    };
    let point = anchor
        .or_else(|| app.mouse_pos.map(|(x, y)| screen_to_canvas(x, y, app)))
        .unwrap_or((app.viewport.x, app.viewport.y));
    let mut item = clipboard.pasted_item();
    place_pasted_item(&mut item, point);
    app.selection.clear();
    app.hovered = None;
    app.interaction = Some(DrawInteraction::Paste { item });
}

fn place_pasted_item(item: &mut DrawItem, point: (f64, f64)) {
    if let Some(bounds) = crate::draw::geometry::transformed_bounds(item) {
        let center = bounds.center();
        item.transform.translate_x += point.0 - center.0;
        item.transform.translate_y += point.1 - center.1;
    }
}

fn begin_rotate(app: &mut DrawAppState, id: crate::draw::state::DrawItemId) {
    let Some(item) = app.data.item(&id) else {
        return;
    };
    if matches!(&item.element, DrawElement::Text(_)) {
        return;
    }
    app.interaction = Some(DrawInteraction::Rotate {
        id,
        pivot_world: (
            item.transform.pivot_x + item.transform.translate_x,
            item.transform.pivot_y + item.transform.translate_y,
        ),
        original_degrees: item.transform.rotation_degrees,
        preview_degrees: item.transform.rotation_degrees,
        start_angle: None,
    });
}

fn begin_scale(app: &mut DrawAppState, id: crate::draw::state::DrawItemId) {
    let Some(item) = app.data.item(&id) else {
        return;
    };
    if matches!(&item.element, DrawElement::Text(_)) {
        return;
    }
    app.interaction = Some(DrawInteraction::Scale {
        id,
        pivot_world: (
            item.transform.pivot_x + item.transform.translate_x,
            item.transform.pivot_y + item.transform.translate_y,
        ),
        original_scale: item.transform.scale,
        preview_scale: item.transform.scale,
        start_distance: None,
    });
}

fn erase_item(app: &mut DrawAppState, id: &crate::draw::state::DrawItemId) -> anyhow::Result<()> {
    if app.data.item(id).is_none() {
        return Ok(());
    }
    let previous = app.data.clone();
    app.data.elements.retain(|item| item.id != *id);
    app.selection.clear();
    app.hovered = None;
    app.commit_data_change(previous)?;
    Ok(())
}

fn set_item_color(
    app: &mut DrawAppState,
    id: &crate::draw::state::DrawItemId,
    color: (u8, u8, u8),
) -> anyhow::Result<()> {
    let Some(item) = app.data.item(id) else {
        return Ok(());
    };
    let previous_color = element_color(&item.element);
    if previous_color == color {
        return Ok(());
    }
    let previous = app.data.clone();
    if let Some(item) = app.data.item_mut(id) {
        set_element_color(&mut item.element, color);
        app.commit_data_change(previous)?;
    }
    Ok(())
}

fn element_color(element: &DrawElement) -> (u8, u8, u8) {
    match element {
        DrawElement::Stroke(stroke) => stroke.color,
        DrawElement::Shape(
            Shape::Rect { color, .. }
            | Shape::Ellipse { color, .. }
            | Shape::Diamond { color, .. }
            | Shape::Line { color, .. }
            | Shape::Arrow { color, .. },
        ) => *color,
        DrawElement::Text(text) => text.color,
    }
}

fn set_element_color(element: &mut DrawElement, color: (u8, u8, u8)) {
    match element {
        DrawElement::Stroke(stroke) => stroke.color = color,
        DrawElement::Shape(
            Shape::Rect { color: current, .. }
            | Shape::Ellipse { color: current, .. }
            | Shape::Diamond { color: current, .. }
            | Shape::Line { color: current, .. }
            | Shape::Arrow { color: current, .. },
        ) => *current = color,
        DrawElement::Text(text) => text.color = color,
    }
}

fn handle_transform_or_paste_mouse(
    ev: MouseEvent,
    app: &mut DrawAppState,
) -> anyhow::Result<Option<DrawEventAction>> {
    let point = screen_to_canvas(ev.column, ev.row, app);
    if matches!(&app.interaction, Some(DrawInteraction::Paste { .. })) {
        match ev.kind {
            MouseEventKind::Moved => update_paste_position(app, point),
            MouseEventKind::Down(MouseButton::Left) => {
                update_paste_position(app, point);
                commit_paste(app)?;
            }
            _ => {}
        }
        return Ok(None);
    }

    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => begin_transform_drag(app, point),
        MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Moved => {
            update_transform_preview(
                app,
                point,
                ev.modifiers.contains(crossterm::event::KeyModifiers::SHIFT),
            );
        }
        MouseEventKind::Up(MouseButton::Left) => finish_transform(app)?,
        _ => {}
    }
    Ok(None)
}

fn update_paste_position(app: &mut DrawAppState, point: (f64, f64)) {
    if let Some(DrawInteraction::Paste { item }) = &mut app.interaction {
        place_pasted_item(item, point);
    }
}

fn commit_paste(app: &mut DrawAppState) -> anyhow::Result<()> {
    let Some(DrawInteraction::Paste { item }) = app.interaction.take() else {
        return Ok(());
    };
    let id = item.id.clone();
    let previous = app.data.clone();
    app.data.elements.push(item);
    app.selection.select_only(id);
    app.commit_data_change(previous)?;
    Ok(())
}

fn begin_transform_drag(app: &mut DrawAppState, point: (f64, f64)) {
    match &mut app.interaction {
        Some(DrawInteraction::Rotate {
            pivot_world,
            start_angle,
            ..
        }) => {
            *start_angle = Some((point.1 - pivot_world.1).atan2(point.0 - pivot_world.0));
        }
        Some(DrawInteraction::Scale {
            pivot_world,
            start_distance,
            ..
        }) => {
            *start_distance = Some((point.0 - pivot_world.0).hypot(point.1 - pivot_world.1));
        }
        _ => {}
    }
}

fn update_transform_preview(app: &mut DrawAppState, point: (f64, f64), snap: bool) {
    match &mut app.interaction {
        Some(DrawInteraction::Rotate {
            pivot_world,
            original_degrees,
            preview_degrees,
            start_angle: Some(start_angle),
            ..
        }) => {
            let current = (point.1 - pivot_world.1).atan2(point.0 - pivot_world.0);
            let mut delta = (current - *start_angle).to_degrees();
            if snap {
                delta = (delta / 15.0).round() * 15.0;
            }
            *preview_degrees = (*original_degrees + delta).rem_euclid(360.0);
        }
        Some(DrawInteraction::Scale {
            pivot_world,
            original_scale,
            preview_scale,
            start_distance: Some(start_distance),
            ..
        }) if *start_distance > f64::EPSILON => {
            let current = (point.0 - pivot_world.0).hypot(point.1 - pivot_world.1);
            *preview_scale = (*original_scale * current / *start_distance).clamp(0.1, 10.0);
        }
        _ => {}
    }
}

fn finish_transform(app: &mut DrawAppState) -> anyhow::Result<()> {
    let Some(interaction) = app.interaction.take() else {
        return Ok(());
    };
    match interaction {
        paused @ (DrawInteraction::Rotate {
            start_angle: None, ..
        }
        | DrawInteraction::Scale {
            start_distance: None,
            ..
        }) => {
            app.interaction = Some(paused);
        }
        DrawInteraction::Rotate {
            id,
            original_degrees,
            preview_degrees,
            ..
        } if original_degrees != preview_degrees => {
            let previous = app.data.clone();
            if let Some(item) = app.data.item_mut(&id) {
                item.transform.rotation_degrees = preview_degrees;
                app.commit_data_change(previous)?;
            }
        }
        DrawInteraction::Scale {
            id,
            original_scale,
            preview_scale,
            ..
        } if original_scale != preview_scale => {
            let previous = app.data.clone();
            if let Some(item) = app.data.item_mut(&id) {
                item.transform.scale = preview_scale;
                app.commit_data_change(previous)?;
            }
        }
        DrawInteraction::Move { .. } | DrawInteraction::Paste { .. } => {}
        DrawInteraction::Rotate { .. } | DrawInteraction::Scale { .. } => {}
    }
    Ok(())
}

fn cursor_left_down(mouse: MouseEvent, point: (f64, f64), app: &mut DrawAppState) {
    let selected_handles = app.selection.primary.as_ref().and_then(|id| {
        app.data
            .item(id)
            .filter(|item| !matches!(&item.element, DrawElement::Text(_)))
            .and_then(|item| {
                crate::draw::geometry::selection_handle_points(item, &item.transform, &app.viewport)
                    .map(|handles| (id.clone(), handles))
            })
    });
    if let Some((id, (rotation, scale))) = selected_handles {
        let tolerance = 5.0 / app.viewport.zoom.abs();
        if (point.0 - rotation.0).hypot(point.1 - rotation.1) <= tolerance {
            begin_rotate(app, id);
            begin_transform_drag(app, point);
            return;
        }
        if (point.0 - scale.0).hypot(point.1 - scale.1) <= tolerance {
            begin_scale(app, id);
            begin_transform_drag(app, point);
            return;
        }
    }

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
        app.last_mouse_pos = Some((mouse.column, mouse.row));
    }
    app.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
}

fn cursor_left_drag(mouse: MouseEvent, point: (f64, f64), app: &mut DrawAppState) {
    let Some(DrawInteraction::Move {
        start_world,
        original_translation,
        preview_translation,
        ..
    }) = &mut app.interaction
    else {
        if app.last_mouse_pos.is_some() {
            panning(mouse.column, mouse.row, app);
        }
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
    use crossterm::event::{KeyEvent, KeyModifiers};
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

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
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
        let mut clipboard = None;

        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 50, 50),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Drag(MouseButton::Left), 60, 60),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
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
            &mut clipboard,
        )
        .unwrap();
        let item = state.data.item(&id).unwrap();
        assert!((item.transform.translate_x - 20.0).abs() < 1e-9);
        assert!((item.transform.translate_y + 20.0).abs() < 1e-9);
        assert_eq!(state.undo_stack.len(), 1);
        assert!(state.interaction.is_none());
    }

    #[test]
    fn cursor_empty_drag_pans_view() {
        let (_temp, mut state) = test_state();
        state.last_area = Rect::new(0, 0, 100, 100);
        let keybinds = Keybinds::default();
        let config = crate::config::ClinConfig::default();
        let mut clipboard = None;

        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 20, 20),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Drag(MouseButton::Left), 30, 25),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();

        assert!(state.is_panning);
        assert_eq!(state.viewport.x, -20.0);
        assert_eq!(state.viewport.y, 10.0);
        assert!(state.undo_stack.is_empty());

        handle_event(
            mouse(MouseEventKind::Up(MouseButton::Left), 30, 25),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();

        assert!(!state.is_panning);
        assert!(state.last_mouse_pos.is_none());
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
        let mut clipboard = None;

        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 50, 49),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Up(MouseButton::Left), 50, 49),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 50, 49),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();

        assert_eq!(
            state.text_editor.as_ref().map(|(target, _)| target),
            Some(&id)
        );
        assert!(state.interaction.is_none());
    }

    #[test]
    fn context_menu_colors_copies_and_pastes_fresh_item() {
        let (_temp, mut state) = test_state();
        state.last_area = Rect::new(0, 0, 100, 100);
        let source = DrawItem::new(DrawElement::Shape(Shape::Rect {
            x: -2.0,
            y: -2.0,
            width: 4.0,
            height: 4.0,
            color: (255, 255, 255),
        }));
        let source_id = source.id.clone();
        state.data.elements.push(source);
        let keybinds = Keybinds::default();
        let config = crate::config::ClinConfig::default();
        let mut clipboard = None;

        for event in [
            mouse(MouseEventKind::Down(MouseButton::Right), 50, 50),
            mouse(MouseEventKind::Up(MouseButton::Right), 50, 50),
        ] {
            handle_event(event, &mut state, &keybinds, &config, &mut clipboard).unwrap();
        }
        assert_eq!(
            state
                .context_menu
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|item| item.label)
                .collect::<Vec<_>>(),
            vec!["Rotate", "Scale", "Color...", "Copy", "Erase"]
        );

        handle_event(
            key(KeyCode::Char('o')),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();
        assert_eq!(state.menu_kind, Some(DrawMenuKind::Color));
        handle_event(
            key(KeyCode::Char('r')),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();
        assert_eq!(
            element_color(&state.data.item(&source_id).unwrap().element),
            (255, 82, 82)
        );

        for event in [
            mouse(MouseEventKind::Down(MouseButton::Right), 50, 50),
            mouse(MouseEventKind::Up(MouseButton::Right), 50, 50),
            key(KeyCode::Char('c')),
            mouse(MouseEventKind::Down(MouseButton::Right), 70, 50),
            mouse(MouseEventKind::Up(MouseButton::Right), 70, 50),
        ] {
            handle_event(event, &mut state, &keybinds, &config, &mut clipboard).unwrap();
        }
        assert_eq!(
            state
                .context_menu
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|item| item.label)
                .collect::<Vec<_>>(),
            vec!["Paste"]
        );
        assert_eq!(
            clipboard.as_ref().map(|saved| &saved.item.id),
            Some(&source_id)
        );

        handle_event(
            key(KeyCode::Char('p')),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 70, 50),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();

        assert_eq!(state.data.elements.len(), 2);
        let pasted = state.data.elements.last().unwrap();
        assert_ne!(pasted.id, source_id);
        let center = crate::draw::geometry::transformed_bounds(pasted)
            .expect("pasted draw item has bounds")
            .center();
        assert!((center.0 - 41.0).abs() < 1e-9);
        assert!((center.1 + 1.0).abs() < 1e-9);
        assert_eq!(state.selection.primary, Some(pasted.id.clone()));
        assert_eq!(state.undo_stack.len(), 2);
    }

    #[test]
    fn rotate_and_scale_preview_then_commit_once_each() {
        let (_temp, mut state) = test_state();
        let item = DrawItem::new(DrawElement::Shape(Shape::Rect {
            x: -1.0,
            y: -1.0,
            width: 2.0,
            height: 2.0,
            color: (255, 255, 255),
        }));
        let id = item.id.clone();
        state.data.elements.push(item);
        state.selection.select_only(id.clone());
        cursor_left_down(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            (0.0, 9.0),
            &mut state,
        );
        assert!(matches!(
            &state.interaction,
            Some(DrawInteraction::Rotate {
                id: selected_id,
                start_angle: Some(_),
                ..
            }) if selected_id == &id
        ));
        state.interaction = None;

        begin_rotate(&mut state, id.clone());
        begin_transform_drag(&mut state, (1.0, 0.0));
        update_transform_preview(&mut state, (0.0, 1.0), false);
        finish_transform(&mut state).unwrap();
        assert!((state.data.item(&id).unwrap().transform.rotation_degrees - 90.0).abs() < 1e-9);

        begin_scale(&mut state, id.clone());
        begin_transform_drag(&mut state, (1.0, 0.0));
        update_transform_preview(&mut state, (20.0, 0.0), false);
        finish_transform(&mut state).unwrap();
        assert_eq!(state.data.item(&id).unwrap().transform.scale, 10.0);
        assert_eq!(state.undo_stack.len(), 2);
    }

    #[test]
    fn cursor_and_grid_keybinds_update_transient_draw_state() {
        let (_temp, mut state) = test_state();
        let keybinds = Keybinds::default();
        let config = crate::config::ClinConfig::default();
        let mut clipboard = None;
        state.active_tool = DrawTool::Draw;

        handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE)),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();
        assert_eq!(state.active_tool, DrawTool::Cursor);

        handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT)),
            &mut state,
            &keybinds,
            &config,
            &mut clipboard,
        )
        .unwrap();
        assert!(!state.grid.visible);
    }
}
