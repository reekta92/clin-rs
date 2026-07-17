use ratatui::{prelude::*, widgets::*};

use super::{
    PopupHints, PopupSize, centered_rect, draw_dim_vline, draw_popup_frame, draw_status_bar,
    draw_view_title_bar, format_keybind_hints, get_preview_info,
};
use crate::app::{App, EditFocus, EditMode, EditSidebar, ViewMode};
use crate::content_tree::parse::NodeKind;
use crate::events::get_title_text;
use crate::keybinds::EditAction;

/// Render the body editor widget with proper style, cursor, line numbers, and cursor-line fill.
/// Called from both the preview and non-preview paths to eliminate a ~50‑line duplication.
pub(crate) fn render_editor_widget(
    frame: &mut Frame,
    app: &mut App,
    focus: EditFocus,
    area: Rect,
    custom_block: Option<Block<'static>>,
    custom_style: Option<Style>,
) {
    let block = custom_block.unwrap_or_else(|| {
        Block::default()
            .style(app.app_theme.bg_style())
            .borders(Borders::NONE)
            .padding(Padding::new(0, 2, 0, 0))
    });
    let base_style = custom_style.unwrap_or_else(|| app.app_theme.bg_style());
    super::render_textarea_with_theme(
        frame,
        &mut app.editor.editor,
        area,
        &app.app_theme,
        focus == EditFocus::Body,
        app.editor.show_line_numbers,
        block,
        base_style,
    );
    if app
        .editor
        .find_popup
        .as_ref()
        .is_some_and(|p| !p.query().is_empty())
    {
        super::overlay_search_highlights(frame, app, area);
    }
}

/// Render the editor body in READ mode (rendered markdown, scrollable).
pub(crate) fn render_read_view(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.editor.read_dirty || app.editor.read_cols != area.width {
        app.refresh_read_mode();
    }
    // Clamp scroll offset
    let max_offset = app
        .editor
        .read_grid
        .len()
        .saturating_sub(area.height as usize);
    app.editor.read_offset = app.editor.read_offset.min(max_offset);
    let grid: &[Vec<(char, Style)>] = &app.editor.read_grid;
    let snap = crate::snapshot::RenderedSnapshot::new(grid)
        .scroll_offset(app.editor.read_offset as u16)
        .block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(0, 2, 0, 0)),
        );
    frame.render_widget(snap, area);
}

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

    // Prepend mode indicator to the left header bar
    let mode_indicator = Span::styled(
        match app.editor.edit_mode {
            EditMode::Read => " READ ",
            EditMode::Edit => " EDIT ",
        },
        Style::default()
            .fg(match app.editor.edit_mode {
                EditMode::Read => app.app_theme.muted,
                EditMode::Edit => app.app_theme.success,
            })
            .add_modifier(ratatui::style::Modifier::BOLD),
    );
    let mut header_spans = vec![mode_indicator];
    header_spans.extend(left_line.spans.clone());
    let left_line = ratatui::text::Line::from(header_spans);
    let status_val = Some(app.status.as_ref());
    let has_status = if let Some(st) = status_val {
        !st.trim().is_empty() && st != "Ready"
    } else {
        false
    };

    if has_status {
        draw_view_title_bar(
            frame,
            outer_chunks[0],
            &app.app_theme,
            left_line,
            right_line,
            status_val,
        );
        app.editor.header_title_rect = Rect::default();
    } else {
        let left_width = left_line.width() as u16;
        let right_width = right_line.as_ref().map(|r| r.width() as u16).unwrap_or(0);

        let header_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_width),
                Constraint::Min(0),
                Constraint::Length(right_width),
            ])
            .split(outer_chunks[0]);

        let left_area = header_chunks[0];
        let center_area = header_chunks[1];
        let right_area = header_chunks[2];

        let theme = &app.app_theme;
        let title_str = get_title_text(&app.editor.title_editor).into_owned();

        if focus == EditFocus::Title {
            // Render background/blank bar first
            let background_bar = Paragraph::new("").style(theme.title_bar_bg_style());
            frame.render_widget(background_bar, outer_chunks[0]);

            // Render left and right bars on top of the background
            let left_bar = Paragraph::new(left_line).style(theme.title_bar_bg_style());
            frame.render_widget(left_bar, left_area);

            if let Some(r_text) = right_line {
                let is_powerline = matches!(
                    theme.hint_bar_style,
                    crate::config::HintBarStyle::Sharp
                        | crate::config::HintBarStyle::Rounded
                        | crate::config::HintBarStyle::Slanted
                );
                if is_powerline {
                    let r_bar = Paragraph::new(r_text)
                        .style(theme.hint_line_bg_style())
                        .alignment(Alignment::Left);
                    frame.render_widget(r_bar, right_area);
                } else {
                    let r_bar = Paragraph::new(r_text)
                        .style(theme.title_bar_bg_style())
                        .alignment(Alignment::Right);
                    frame.render_widget(r_bar, right_area);
                }
            }

            // Render TextArea centered relative to the screen width, clamped to left/right bars
            let text_width = title_str.len() as u16;
            let display_width = text_width.min(outer_chunks[0].width);
            let raw_x_offset = (outer_chunks[0].width.saturating_sub(display_width)) / 2;
            let start_x = (outer_chunks[0].x + raw_x_offset).max(left_area.right());
            let end_x = right_area.x;
            let title_rect =
                Rect::new(start_x, outer_chunks[0].y, end_x.saturating_sub(start_x), 1);

            app.editor
                .title_editor
                .set_style(theme.title_bar_bg_style().fg(theme.heading));
            app.editor.title_editor.set_block(
                Block::default()
                    .style(theme.title_bar_bg_style())
                    .borders(Borders::NONE),
            );
            app.editor
                .title_editor
                .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            app.editor
                .title_editor
                .set_cursor_line_style(Style::default());
            frame.render_widget(&app.editor.title_editor, title_rect);
        } else {
            // Render background + centered title Paragraph on outer_chunks[0]
            let (span, style) = if title_str.is_empty() {
                ("Untitled note", Style::default().fg(theme.muted))
            } else {
                (title_str.as_str(), Style::default().fg(theme.heading))
            };
            let center_paragraph = Paragraph::new(Line::from(vec![Span::styled(span, style)]))
                .style(theme.title_bar_bg_style())
                .alignment(Alignment::Center);
            frame.render_widget(center_paragraph, outer_chunks[0]);

            // Render left and right bars on top of the centered title
            let left_bar = Paragraph::new(left_line).style(theme.title_bar_bg_style());
            frame.render_widget(left_bar, left_area);

            if let Some(r_text) = right_line {
                let is_powerline = matches!(
                    theme.hint_bar_style,
                    crate::config::HintBarStyle::Sharp
                        | crate::config::HintBarStyle::Rounded
                        | crate::config::HintBarStyle::Slanted
                );
                if is_powerline {
                    let r_bar = Paragraph::new(r_text)
                        .style(theme.hint_line_bg_style())
                        .alignment(Alignment::Left);
                    frame.render_widget(r_bar, right_area);
                } else {
                    let r_bar = Paragraph::new(r_text)
                        .style(theme.title_bar_bg_style())
                        .alignment(Alignment::Right);
                    frame.render_widget(r_bar, right_area);
                }
            }
        }

        app.editor.header_title_rect = center_area;
    }
    let body_area = outer_chunks[1];
    let hint_area = outer_chunks[2];

    let layout = crate::events::compute_edit_layout(
        body_area,
        app.preview_fullscreen,
        app.editor.editor_preview_enabled,
        app.editor.sidebar,
        app.preview_position,
    );

    if app.preview_fullscreen {
        app.editor.last_preview_pane_width = body_area.width;
        app.editor.last_preview_pane_height = body_area.height;
    } else if app.editor.editor_preview_enabled
        && app.editor.sidebar == EditSidebar::None
        && let Some(p) = layout.preview
    {
        app.editor.last_preview_pane_width = p.width;
        app.editor.last_preview_pane_height = p.height;
    }

    let sidebar_area = layout.sidebar;
    let preview_area_rect = layout.preview;
    let splitter_area = layout.splitter;

    let editor_container = layout.body;

    app.editor.last_body_width = editor_container.width;
    app.editor.last_body_height = editor_container.height;

    if let Some(sb) = sidebar_area {
        draw_sidebar_pane(frame, sb, app, focus);
    }
    if let Some(preview_area_rect) = preview_area_rect {
        if !app.preview_fullscreen {
            match app.editor.edit_mode {
                EditMode::Read => render_read_view(frame, app, editor_container),
                EditMode::Edit => {
                    render_editor_widget(frame, app, focus, editor_container, None, None);
                    super::overlay_markdown_highlight(frame, app, editor_container);
                }
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
                } // closes if let Some(page_grid)
            } // closes if !renderer.is_pending()
        } // closes if let Some(renderer)
    } else {
        match app.editor.edit_mode {
            EditMode::Read => render_read_view(frame, app, editor_container),
            EditMode::Edit => {
                render_editor_widget(frame, app, focus, editor_container, None, None);
                super::overlay_markdown_highlight(frame, app, editor_container);
            }
        }
    }
    let kb = &app.keybinds;
    let mode_hint = match app.editor.edit_mode {
        EditMode::Read => ("e".to_string(), "edit"),
        EditMode::Edit => (kb.display_edit(EditAction::Back), "read"),
    };
    let hints_items = vec![
        mode_hint,
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
    if let Some(popup) = &app.editor.find_popup {
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
    // --- Go-to-line popup ---
    if let Some(input) = &app.editor.go_to_line_input {
        let theme = &app.app_theme;
        let popup_area =
            crate::ui::popups::centered_rect(crate::ui::PopupSize::Prompt, frame.area());
        let text = if input.is_empty() { "Line #" } else { input };
        let (title, hints) = ("GO TO LINE", &[("Enter", "jump"), ("Esc", "cancel")][..]);
        let clear = Clear;
        frame.render_widget(clear, popup_area);
        let block = Block::default()
            .title(format!(" {title} "))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.highlight_fg))
            .style(theme.bg_style());
        frame.render_widget(&block, popup_area);
        let inner = block.inner(popup_area);
        let input_style = Style::default().fg(theme.highlight_fg).bg(theme.accent);
        let paragraph = Paragraph::new(Span::styled(text.to_string(), input_style)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border)),
        );
        frame.render_widget(paragraph, inner);
        // Render hints
        if inner.height >= 1 {
            let hint_y = inner.y + inner.height.saturating_sub(1);
            let hint_area = Rect::new(inner.x, hint_y, inner.width, 1);
            let hint_line = Line::from(
                hints
                    .iter()
                    .flat_map(|(k, v)| {
                        let key = Span::styled(
                            format!(" {k} "),
                            Style::default().fg(theme.highlight_fg).bg(theme.accent),
                        );
                        let desc = Span::styled(format!(" {v} "), Style::default().fg(theme.text));
                        [key, desc].into_iter()
                    })
                    .collect::<Vec<_>>(),
            );
            frame.render_widget(hint_line, hint_area);
        }
    }
}
fn draw_sidebar_pane(frame: &mut Frame, area: Rect, app: &mut App, focus: EditFocus) {
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

    let (title, items) = match app.editor.sidebar {
        EditSidebar::Outline => {
            let title = format!("  OUTLINE ({})", app.editor.outline_nodes.len());
            let items: Vec<ListItem> = app
                .editor
                .outline_nodes
                .iter()
                .map(|node| {
                    let text = match &node.kind {
                        NodeKind::Header { level, title } => {
                            let indent = "  ".repeat((*level as usize).saturating_sub(1));
                            let marker = if *level == 1 { "▸ " } else { "" };
                            format!("{}{}{}", indent, marker, title)
                        }
                        _ => node.full_text().to_string(),
                    };
                    ListItem::new(text).style(Style::default().fg(theme.fg))
                })
                .collect();
            (title, items)
        }
        EditSidebar::Links => {
            let title = format!("  LINKS ({})", app.editor.links.len());
            let items: Vec<ListItem> = app
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
            (title, items)
        }
        EditSidebar::None => return,
    };

    let title_widget = Paragraph::new(title).style(title_style);
    frame.render_widget(title_widget, sb_chunks[1]);

    if items.is_empty() {
        let empty_msg = match app.editor.sidebar {
            EditSidebar::Outline => "  No headers",
            EditSidebar::Links => "  No links",
            EditSidebar::None => "",
        };
        let p = Paragraph::new(empty_msg).style(Style::default().fg(theme.muted));
        frame.render_widget(p, sb_chunks[3]);
    } else {
        let mut state = crate::ui::list_state_selected(
            Some(app.editor.sidebar_selected),
            app.editor.sidebar_scroll_offset,
        );

        // Add left padding to the list rect to align items nicely
        let list_area = Rect::new(
            sb_chunks[3].x + 2,
            sb_chunks[3].y,
            sb_chunks[3].width.saturating_sub(2),
            sb_chunks[3].height,
        );

        let item_count = items.len();
        let list = List::new(items).block(Block::default()).highlight_style(
            Style::default()
                .bg(theme.highlight_bg)
                .fg(theme.highlight_fg),
        );
        frame.render_stateful_widget(list, list_area, &mut state);
        app.editor.sidebar_scroll_offset = state.offset();
        app.editor.sidebar_list_rect = list_area;
        crate::ui::paint_list_hover(
            frame,
            list_area,
            &state,
            item_count,
            app.mouse_pos,
            theme.hover_style(),
        );
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
