use ratatui::{prelude::*, widgets::*};

use super::{
    PopupHints, PopupSize, centered_rect, draw_corner_watermark, draw_dim_vline, draw_popup_frame,
    draw_status_bar, draw_view_title_bar, fill_cursor_line_bg, format_keybind_hints,
    get_preview_info, get_textarea_scroll, line_number_gutter,
};
use crate::app::{App, EditFocus, EditSidebar, ViewMode};
use crate::events::get_title_text;
use crate::keybinds::EditAction;

#[allow(clippy::collapsible_if)]
pub fn draw_edit_view(frame: &mut Frame, app: &mut App, focus: EditFocus) {
    let area = frame.area();

    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let preview_info = get_preview_info(app);
    let note = crate::statusline::active_note(app, ViewMode::Edit);
    let mut ctx = crate::statusline::StatuslineContext::for_view(app, ViewMode::Edit);
    ctx.area = Some(outer_chunks[0]);
    ctx.note = note;
    ctx.preview_info = preview_info.as_ref();
    if let Some(pi) = &preview_info {
        ctx.preview = Some(super::preview_spans(pi, &app.app_theme));
    }

    let (left_line, right_line) = crate::statusline::render_header(
        &ctx,
        &app.config.statusline,
        ViewMode::Edit,
        &app.app_theme,
    );
    draw_view_title_bar(
        frame,
        outer_chunks[0],
        &app.app_theme,
        left_line,
        right_line,
        Some(app.status.as_ref()),
    );

    let body_area = outer_chunks[1];
    let hint_area = outer_chunks[2];

    let mut sidebar_area = None;
    let (edit_area, preview_area_rect, splitter_area) = if app.preview_fullscreen {
        app.editor.last_preview_pane_width = body_area.width;
        app.editor.last_preview_pane_height = body_area.height;
        (body_area, Some(body_area), None)
    } else if app.editor.sidebar != EditSidebar::None {
        let (constraints, main_idx, sb_idx) = match app.preview_position {
            crate::config::PreviewPosition::Left => (
                [
                    Constraint::Ratio(30, 100),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ],
                2,
                0,
            ),
            crate::config::PreviewPosition::Right => (
                [
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Ratio(30, 100),
                ],
                0,
                2,
            ),
        };
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(body_area);
        sidebar_area = Some(cols[sb_idx]);
        (cols[main_idx], None, Some(cols[1]))
    } else if app.editor.editor_preview_enabled {
        let (constraints, main_idx, p_idx) = match app.preview_position {
            crate::config::PreviewPosition::Left => (
                [
                    Constraint::Ratio(43, 100),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ],
                2,
                0,
            ),
            crate::config::PreviewPosition::Right => (
                [
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Ratio(43, 100),
                ],
                0,
                2,
            ),
        };
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(body_area);
        app.editor.last_preview_pane_width = cols[p_idx].width;
        app.editor.last_preview_pane_height = cols[p_idx].height;
        (cols[main_idx], Some(cols[p_idx]), Some(cols[1]))
    } else {
        (body_area, None, None)
    };

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(edit_area);

    let title_area = inner_chunks[0];
    let editor_container = inner_chunks[1];

    if !app.preview_fullscreen {
        app.editor
            .title_editor
            .set_style(app.app_theme.title_bar_bg_style().fg(app.app_theme.heading));
        app.editor.title_editor.set_block(
            Block::default()
                .style(app.app_theme.title_bar_bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(2, 1, 1, 1)),
        );
        app.editor
            .title_editor
            .set_cursor_style(if focus == EditFocus::Title {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            });
        app.editor
            .title_editor
            .set_cursor_line_style(Style::default());
        frame.render_widget(&app.editor.title_editor, title_area);

        if get_title_text(&app.editor.title_editor).is_empty() {
            let title_inner = Rect::new(
                title_area.x + 3,
                title_area.y + 1,
                title_area.width.saturating_sub(4),
                1,
            );
            let placeholder = Paragraph::new(Line::from(Span::styled(
                "Untitled note",
                Style::default().fg(app.app_theme.muted),
            )));
            frame.render_widget(placeholder, title_inner);
        }
    }

    if let Some(sb) = sidebar_area {
        match app.editor.sidebar {
            EditSidebar::Outline => draw_outline_pane(frame, sb, app, focus),
            EditSidebar::Links => draw_links_pane(frame, sb, app, focus),
            EditSidebar::None => {}
        }
    }
    if let Some(preview_area_rect) = preview_area_rect {
        let content_area = editor_container;

        if !app.preview_fullscreen {
            let line_count = app.editor.editor.lines().len();
            let cursor_row = app.editor.editor.cursor().0;
            let scroll_row = get_textarea_scroll(&app.editor.editor).0;

            let editor_area = if app.editor.show_line_numbers {
                let digits = line_count.max(1).to_string().len() as u16;
                let gutter_width = digits + 1;
                let gutter_area = Rect::new(
                    content_area.x,
                    content_area.y,
                    gutter_width.min(content_area.width),
                    content_area.height,
                );
                let gutter = line_number_gutter(
                    line_count,
                    cursor_row,
                    scroll_row,
                    content_area.height,
                    &app.app_theme,
                    0,
                );
                frame.render_widget(gutter, gutter_area);
                Rect::new(
                    content_area.x + gutter_area.width,
                    content_area.y,
                    content_area.width.saturating_sub(gutter_area.width),
                    content_area.height,
                )
            } else {
                content_area
            };

            app.editor.editor.set_block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::NONE)
                    .padding(Padding::new(0, 2, 0, 0)),
            );
            app.editor.editor.set_style(app.app_theme.bg_style());
            app.editor
                .editor
                .set_cursor_style(if focus == EditFocus::Body {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                });
            app.editor
                .editor
                .set_cursor_line_style(if focus == EditFocus::Body {
                    if let Some(bg) = app.app_theme.preview_bg() {
                        Style::default().bg(bg)
                    } else {
                        Style::default()
                            .bg(app.app_theme.highlight_bg)
                            .fg(app.app_theme.highlight_fg)
                    }
                } else {
                    Style::default()
                });
            frame.render_widget(&app.editor.editor, editor_area);
            if focus == EditFocus::Body {
                let cursor_bg = app
                    .app_theme
                    .preview_bg()
                    .unwrap_or(app.app_theme.highlight_bg);
                fill_cursor_line_bg(frame, &app.editor.editor, editor_area, cursor_bg);
            }
        }

        if let Some(renderer) = &app.editor.md_preview_renderer {
            if !renderer.is_pending() && renderer.pages_built() {
                if let Some(page_grid) = renderer.current_page_grid() {
                    let snapshot = crate::snapshot::RenderedSnapshot::new(page_grid).block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                    frame.render_widget(snapshot, preview_area_rect);
                    if renderer.total_pages() > 1 {
                        let indicator = format!(
                            " {}/{} ",
                            renderer.current_page() + 1,
                            renderer.total_pages()
                        );
                        let ind_width = indicator.len() as u16;
                        let ind_x = preview_area_rect.right().saturating_sub(ind_width + 2);
                        let ind_y = preview_area_rect.bottom().saturating_sub(1);
                        if ind_x >= preview_area_rect.x && ind_y >= preview_area_rect.y {
                            let ind_area = Rect::new(ind_x, ind_y, ind_width, 1);
                            let ind_widget = Paragraph::new(Span::styled(
                                indicator,
                                Style::default()
                                    .fg(app.app_theme.muted)
                                    .add_modifier(Modifier::DIM),
                            ));
                            frame.render_widget(ind_widget, ind_area);
                        }
                    }

                    // Overlay decoded images on their reserved lines
                    if let (Some(picker), Some(decode_tx)) =
                        (&app.editor.image_picker, &app.editor.image_decode_tx)
                    {
                        let inner_pad = 2_u16; // left padding of snapshot
                        let col_width = preview_area_rect.width.saturating_sub(2 * inner_pad);
                        let slots = renderer.current_page_image_slots();
                        for (line_idx, url) in slots {
                            let resolved = app.storage.resolve_attachment(url);
                            let path = resolved.unwrap_or_else(|| app.storage.notes_dir.join(url));
                            if !path.exists() {
                                continue;
                            }
                            let key = crate::image_render::ImageKey { path, mtime: 0 };
                            if app.editor.image_cache.get_proto(&key).is_none() {
                                app.editor.image_cache.request(
                                    key.clone(),
                                    2048,
                                    decode_tx,
                                    picker,
                                );
                            }
                            if let Some(proto) = app.editor.image_cache.get_proto(&key) {
                                let row = preview_area_rect.y + 1 + *line_idx as u16;
                                let max_h = app.config.image.preview_rows as u16;
                                let img_rect = Rect::new(
                                    preview_area_rect.x + inner_pad,
                                    row,
                                    col_width.min(preview_area_rect.width.saturating_sub(2)),
                                    max_h.min(preview_area_rect.bottom().saturating_sub(row)),
                                );
                                if img_rect.width > 1 && img_rect.height > 1 {
                                    frame.render_widget(Clear, img_rect);
                                    frame.render_widget(
                                        Block::default().style(app.app_theme.preview_bg_style()),
                                        img_rect,
                                    );
                                    frame.render_stateful_widget(
                                        ratatui_image::StatefulImage::default()
                                            .resize(ratatui_image::Resize::Fit(None)),
                                        img_rect,
                                        proto,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        let line_count = app.editor.editor.lines().len();
        let cursor_row = app.editor.editor.cursor().0;
        let scroll_row = get_textarea_scroll(&app.editor.editor).0;
        let content_area = editor_container;

        let editor_area = if app.editor.show_line_numbers {
            let digits = line_count.max(1).to_string().len() as u16;
            let gutter_width = digits + 1;
            let gutter_area = Rect::new(
                content_area.x,
                content_area.y,
                gutter_width.min(content_area.width),
                content_area.height,
            );
            let gutter = line_number_gutter(
                line_count,
                cursor_row,
                scroll_row,
                content_area.height,
                &app.app_theme,
                0,
            );
            frame.render_widget(gutter, gutter_area);
            Rect::new(
                content_area.x + gutter_area.width,
                content_area.y,
                content_area.width.saturating_sub(gutter_area.width),
                content_area.height,
            )
        } else {
            content_area
        };

        app.editor.editor.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(0, 2, 0, 0)),
        );
        app.editor.editor.set_style(app.app_theme.bg_style());
        app.editor
            .editor
            .set_cursor_style(if focus == EditFocus::Body {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            });
        app.editor
            .editor
            .set_cursor_line_style(if focus == EditFocus::Body {
                if let Some(bg) = app.app_theme.preview_bg() {
                    Style::default().bg(bg)
                } else {
                    Style::default()
                        .bg(app.app_theme.highlight_bg)
                        .fg(app.app_theme.highlight_fg)
                }
            } else {
                Style::default()
            });
        frame.render_widget(&app.editor.editor, editor_area);
        if focus == EditFocus::Body {
            let cursor_bg = app
                .app_theme
                .preview_bg()
                .unwrap_or(app.app_theme.highlight_bg);
            fill_cursor_line_bg(frame, &app.editor.editor, editor_area, cursor_bg);
        }
    }

    let kb = &app.keybinds;
    let hints_items = vec![
        (kb.display_edit(EditAction::CycleFocus), "focus"),
        (kb.display_edit(EditAction::Back), "back"),
        (
            kb.display_edit(EditAction::ToggleMarkdownPreview),
            "preview",
        ),
        (kb.display_edit(EditAction::ToggleOutline), "outline"),
        (kb.display_edit(EditAction::ToggleLinks), "links"),
        (kb.display_edit(EditAction::PreviewLink), "peek link"),
        (kb.display_edit(EditAction::Find), "find"),
        (kb.display_edit(EditAction::InsertDate), "date"),
        (kb.display_edit(EditAction::ToggleSoftWrap), "wrap"),
    ];
    let default_hints = format_keybind_hints(&app.app_theme, &hints_items);
    let hint = default_hints;
    let note = crate::statusline::active_note(app, ViewMode::Edit);
    let mut ctx = crate::statusline::StatuslineContext::for_view(app, ViewMode::Edit);
    ctx.area = Some(hint_area);
    ctx.note = note;
    ctx.hints = Some(hint.spans);
    if let Some(p) = &app.seq_matcher.pending_display() {
        ctx.pending = Some(vec![Span::styled(
            format!("{} ", p),
            Style::default()
                .fg(app.app_theme.highlight_fg)
                .bg(app.app_theme.accent),
        )]);
    }

    let (left_line, right_line) = crate::statusline::render_footer(
        &ctx,
        &app.config.statusline,
        ViewMode::Edit,
        &app.app_theme,
    );
    draw_status_bar(frame, hint_area, &app.app_theme, left_line, right_line);
    draw_corner_watermark(frame, hint_area, app.app_theme.muted);
    if let Some(splitter_area) = splitter_area {
        draw_dim_vline(frame, splitter_area, app.app_theme.muted);
    }

    if app.status.starts_with("Save failed") || app.status.starts_with("Could not open") {
        let popup = centered_rect(PopupSize::Small, area);
        frame.render_widget(Clear, popup);
        let text = Paragraph::new(app.status.as_ref())
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .title("Error"),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(text, popup);
    }
    if app.editor.link_preview {
        draw_link_preview_popup(frame, area, app);
    }
    if let Some(ref popup) = app.editor.find_popup {
        let theme = &app.app_theme;
        let max_visible = 10usize;
        crate::ui::quick_search::draw_quick_search(
            frame,
            area,
            popup,
            theme,
            max_visible,
            |(_, display): &(usize, String),
             is_selected,
             theme: &crate::app_theme::AppThemeColors| {
                let style = if is_selected {
                    Style::default().fg(theme.fg)
                } else {
                    Style::default().fg(theme.highlight_fg)
                };
                let prefix = if is_selected { "▸ " } else { "  " };
                Line::from(Span::styled(format!("{}{}", prefix, display), style))
            },
            app.config.ui.icon_mode,
        );
    }
}
fn draw_outline_pane(frame: &mut Frame, area: Rect, app: &mut App, focus: EditFocus) {
    let theme = &app.app_theme;

    // Fill the background of the sidebar area
    frame.render_widget(Block::default().style(theme.preview_bg_style()), area);

    let sb_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top padding
            Constraint::Length(1), // Title
            Constraint::Length(1), // Spacer
            Constraint::Min(0),    // List
        ])
        .split(area);

    // Draw Title
    let title_style = if focus == EditFocus::Sidebar {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.heading)
            .add_modifier(Modifier::BOLD)
    };
    let title = Paragraph::new(format!("  OUTLINE ({})", app.editor.outline_nodes.len()))
        .style(title_style);
    frame.render_widget(title, sb_chunks[1]);

    // Draw List
    let mut items: Vec<ListItem> = app
        .editor
        .outline_nodes
        .iter()
        .map(|node| {
            let text = match &node.kind {
                crate::content_tree::parse::NodeKind::Header { level, title } => {
                    let indent = "  ".repeat((*level as usize).saturating_sub(1));
                    let marker = if *level == 1 { "▸ " } else { "" };
                    format!("{}{}{}", indent, marker, title)
                }
                _ => node.full_text().to_string(),
            };
            ListItem::new(text).style(Style::default().fg(theme.fg))
        })
        .collect();

    if items.is_empty() {
        let p = Paragraph::new("  No headers").style(Style::default().fg(theme.muted));
        frame.render_widget(p, sb_chunks[3]);
    } else {
        let mut state = ListState::default();
        state.select(Some(app.editor.sidebar_selected));

        // Add left padding to the list rect to align items nicely
        let list_area = Rect::new(
            sb_chunks[3].x + 2,
            sb_chunks[3].y,
            sb_chunks[3].width.saturating_sub(2),
            sb_chunks[3].height,
        );

        let hovered_idx = app.mouse_pos.and_then(|(col, row)| {
            if col >= list_area.x
                && col < list_area.x + list_area.width
                && row >= list_area.y
                && row < list_area.y + list_area.height
            {
                let mouse_y_offset = row - list_area.y;
                let offset = state.offset();
                let idx = mouse_y_offset as usize + offset;
                if idx < items.len() {
                    Some(idx)
                } else {
                    None
                }
            } else {
                None
            }
        });

        if let Some(h_idx) = hovered_idx && Some(h_idx) != state.selected() {
            items[h_idx] = items[h_idx].clone().style(theme.hover_style());
        }

        let list = List::new(items).block(Block::default()).highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .fg(theme.highlight_fg),
        );
        frame.render_stateful_widget(list, list_area, &mut state);
    }
}

fn draw_links_pane(frame: &mut Frame, area: Rect, app: &mut App, focus: EditFocus) {
    let theme = &app.app_theme;

    // Fill the background of the sidebar area
    frame.render_widget(Block::default().style(theme.preview_bg_style()), area);

    let sb_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top padding
            Constraint::Length(1), // Title
            Constraint::Length(1), // Spacer
            Constraint::Min(0),    // List
        ])
        .split(area);

    // Draw Title
    let title_style = if focus == EditFocus::Sidebar {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.heading)
            .add_modifier(Modifier::BOLD)
    };
    let title = Paragraph::new(format!("  LINKS ({})", app.editor.links.len())).style(title_style);
    frame.render_widget(title, sb_chunks[1]);

    // Draw List
    let mut items: Vec<ListItem> = app
        .editor
        .links
        .iter()
        .map(|item| {
            let text = if item.is_backlink {
                format!("←  {}", item.title)
            } else {
                format!("→  {}", item.title)
            };
            ListItem::new(text).style(Style::default().fg(theme.fg))
        })
        .collect();

    if items.is_empty() {
        let p = Paragraph::new("  No links").style(Style::default().fg(theme.muted));
        frame.render_widget(p, sb_chunks[3]);
    } else {
        let mut state = ListState::default();
        state.select(Some(app.editor.sidebar_selected));

        // Add left padding to the list rect to align items nicely
        let list_area = Rect::new(
            sb_chunks[3].x + 2,
            sb_chunks[3].y,
            sb_chunks[3].width.saturating_sub(2),
            sb_chunks[3].height,
        );

        let hovered_idx = app.mouse_pos.and_then(|(col, row)| {
            if col >= list_area.x
                && col < list_area.x + list_area.width
                && row >= list_area.y
                && row < list_area.y + list_area.height
            {
                let mouse_y_offset = row - list_area.y;
                let offset = state.offset();
                let idx = mouse_y_offset as usize + offset;
                if idx < items.len() {
                    Some(idx)
                } else {
                    None
                }
            } else {
                None
            }
        });

        if let Some(h_idx) = hovered_idx && Some(h_idx) != state.selected() {
            items[h_idx] = items[h_idx].clone().style(theme.hover_style());
        }

        let list = List::new(items).block(Block::default()).highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .fg(theme.highlight_fg),
        );
        frame.render_stateful_widget(list, list_area, &mut state);
    }
}
fn draw_link_preview_popup(frame: &mut Frame, area: Rect, app: &mut App) {
    let title = format!(
        " Preview: {} ",
        app.editor.link_preview_target.as_deref().unwrap_or("?")
    );
    let hints = PopupHints::Keybinds(&[("Esc".to_string(), "close")]);
    let inner = draw_popup_frame(frame, area, &title, PopupSize::Large, hints, &app.app_theme);
    if let Some(err) = &app.editor.link_preview_error {
        let p = Paragraph::new(err.as_str())
            .style(Style::default().fg(app.app_theme.destructive))
            .wrap(Wrap { trim: true });
        frame.render_widget(p, inner);
        return;
    }
    let Some(renderer) = &mut app.editor.link_preview_renderer else {
        return;
    };
    if renderer.is_pending() {
        let p = Paragraph::new("Loading…").style(Style::default().fg(app.app_theme.muted));
        frame.render_widget(p, inner);
        return;
    }
    if !renderer.pages_built() {
        return;
    }
    if let Some(grid) = renderer.current_page_grid() {
        let snapshot = crate::snapshot::RenderedSnapshot::new(grid).block(
            Block::default()
                .style(app.app_theme.preview_bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(1, 1, 0, 0)),
        );
        frame.render_widget(snapshot, inner);
    }
}
