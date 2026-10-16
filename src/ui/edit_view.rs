use ratatui::{prelude::*, widgets::*};

use super::{
    PopupHints, PopupSize, centered_rect, draw_dim_vline, draw_popup_frame, draw_status_bar,
    draw_view_title_bar, format_keybind_hints, get_preview_info,
};
use crate::app::{App, EditFocus, EditSidebar, ViewMode};
use crate::keybinds::EditAction;
use crate::outline::parse::NodeKind;
use unicode_width::UnicodeWidthStr;

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
            .padding(Padding::new(0, 2, 1, 0))
    });
    let base_style = custom_style.unwrap_or_else(|| app.app_theme.bg_style());
    let show_ln = app.editor_show_line_numbers();
    super::render_editor_document_with_theme(
        frame,
        &mut app.editor.body,
        area,
        &app.app_theme,
        focus == EditFocus::Body,
        show_ln,
        block,
        base_style,
    );
    {
        let show_ln = app.editor_show_line_numbers();
        let (r, c) = super::refresh_editor_document_viewport(
            &app.editor.body,
            app.editor.body_viewport_row,
            app.editor.body_viewport_col,
            area,
            show_ln,
        );
        app.editor.body_viewport_row = r;
        app.editor.body_viewport_col = c;
        let width = app.editor.body.inner_rect(area).width;
        super::ensure_editor_visual_rows(&mut app.editor, width, show_ln);
    }
    if app
        .editor
        .find_popup
        .as_ref()
        .is_some_and(|p| !p.query().is_empty())
    {
        super::overlay_search_highlights(frame, app, area);
    }
}

/// Inline `[[...]]` note-title suggestions, anchored to the bottom-left of
/// the body area. `Up`/`Down` move, `Tab`/`Enter` accept, `Esc` dismisses,
/// clicking a row accepts it. Records its rect for mouse hit-testing.
fn draw_link_completion(frame: &mut Frame, body_area: Rect, app: &mut App) {
    let Some(comp) = app.editor.link_complete.clone() else {
        app.editor.link_complete_rect = Rect::default();
        return;
    };
    if body_area.width < 24 || body_area.height < 5 {
        app.editor.link_complete_rect = Rect::default();
        return;
    }
    let title = format!("[[{}", comp.query);
    let widest = comp
        .matches
        .iter()
        .map(|(_, t)| t.width())
        .max()
        .unwrap_or(0)
        .max(title.width())
        .max("No matching notes".width());
    let width = ((widest as u16) + 6).clamp(20, body_area.width);
    // Suggestion rows only: a plain bordered overlay with no footer bar.
    // Keybinds live in the help overlay (F1) and the editor hint bar.
    let rows = comp.matches.len().max(1) as u16;
    let height = (rows + 2).min(body_area.height);
    let popup = Rect::new(
        body_area.x,
        body_area.bottom().saturating_sub(height),
        width,
        height,
    );
    app.editor.link_complete_rect = popup;
    let theme = &app.app_theme;
    let block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(format!(" {title} "));
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    let mut lines: Vec<Line> = comp
        .matches
        .iter()
        .enumerate()
        .map(|(i, (_, note_title))| {
            let selected = i == comp.selected;
            let style = if selected {
                Style::default().fg(theme.highlight_fg).bg(theme.heading)
            } else {
                Style::default().fg(theme.text)
            };
            let marker = if selected { "▸ " } else { "  " };
            Line::from(Span::styled(format!("{marker}{note_title}"), style))
        })
        .collect();
    if comp.matches.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No matching notes",
            Style::default().fg(theme.muted),
        )));
    }
    let list_height = inner.height as usize;
    frame.render_widget(
        Paragraph::new(lines.into_iter().take(list_height).collect::<Vec<_>>()),
        inner,
    );
}

#[allow(clippy::collapsible_if)]
pub fn draw_edit_view(frame: &mut Frame, app: &mut App, focus: EditFocus) {
    let area = frame.area();

    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // merged header + tab bar
            Constraint::Min(0),    // body
            Constraint::Length(1), // hint bar
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
    let status_val = Some(app.status.as_ref());
    let has_status = if let Some(st) = status_val {
        !st.trim().is_empty() && st != "Ready"
    } else {
        false
    };

    if app.zen_mode && focus != EditFocus::Title && !has_status {
        let blank = Paragraph::new("").style(app.app_theme.bg_style());
        frame.render_widget(blank, outer_chunks[0]);
        app.editor.header_title_rect = Rect::default();
        app.editor.tab_bar_rect = Rect::default();
        app.editor.tab_bar_rects.clear();
        app.editor.tab_close_rects.clear();
        app.editor.new_tab_rect = Rect::default();
    } else if has_status {
        draw_view_title_bar(
            frame,
            outer_chunks[0],
            &app.app_theme,
            left_line,
            right_line,
            status_val,
            app.load_spinner_tick,
        );
        app.editor.header_title_rect = Rect::default();
        app.editor.tab_bar_rect = Rect::default();
        app.editor.tab_bar_rects.clear();
        app.editor.tab_close_rects.clear();
        app.editor.new_tab_rect = Rect::default();
    } else {
        // Merged header + tab bar in a single row:
        // `[status-left][tabs · active tab carries the title][status-right]`.
        let theme = &app.app_theme;
        frame.render_widget(
            Block::default().style(theme.title_bar_bg_style()),
            outer_chunks[0],
        );
        let left_width = (left_line.width() as u16).min(outer_chunks[0].width);
        let right_width = right_line
            .as_ref()
            .map(|r| r.width() as u16)
            .unwrap_or(0)
            .min(outer_chunks[0].width.saturating_sub(left_width));
        let left_area = Rect::new(outer_chunks[0].x, outer_chunks[0].y, left_width, 1);
        let right_area = Rect::new(
            outer_chunks[0].x + outer_chunks[0].width.saturating_sub(right_width),
            outer_chunks[0].y,
            right_width,
            1,
        );
        let strip = Rect::new(
            left_area.right(),
            outer_chunks[0].y,
            outer_chunks[0]
                .width
                .saturating_sub(left_width)
                .saturating_sub(right_width),
            1,
        );
        let left_bar = Paragraph::new(left_line).style(theme.title_bar_bg_style());
        frame.render_widget(left_bar, left_area);
        if let Some(r_text) = right_line {
            let is_powerline = theme.hint_bar_style.has_filled_cells();
            let r_bar = if is_powerline {
                Paragraph::new(r_text)
                    .style(theme.hint_line_bg_style())
                    .alignment(Alignment::Left)
            } else {
                Paragraph::new(r_text)
                    .style(theme.title_bar_bg_style())
                    .alignment(Alignment::Right)
            };
            frame.render_widget(r_bar, right_area);
        }
        // `left_line`/`right_line` borrows end here; the tab strip below
        // mutates the editor.
        draw_tab_strip(frame, strip, &mut app.editor, &app.app_theme, focus);
    }
    let body_area = outer_chunks[1];
    let hint_area = outer_chunks[2];

    // Focus (zen) mode hides the file sidebar so only the text remains.
    let file_tree_visible = app.editor.file_tree_visible && !app.zen_mode;
    let layout = crate::events::compute_edit_layout(
        body_area,
        app.preview_fullscreen,
        app.editor.editor_preview_enabled,
        app.editor.sidebar,
        app.preview_position,
        app.zen_padding(),
        file_tree_visible,
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
    let file_tree_area = layout.file_tree;
    let file_tree_splitter = layout.file_tree_splitter;

    let editor_container = layout.body;

    app.editor.last_body_width = editor_container.width;
    app.editor.last_body_height = editor_container.height;
    app.editor.file_tree_rect = file_tree_area.unwrap_or_default();

    if let Some(ft) = file_tree_area {
        draw_file_tree_pane(frame, ft, app, focus);
    }
    if let Some(split) = file_tree_splitter {
        draw_dim_vline(frame, split, app.app_theme.muted);
    }
    if let Some(sb) = sidebar_area {
        draw_sidebar_pane(frame, sb, app, focus);
    }
    if !app.preview_fullscreen {
        render_editor_widget(frame, app, focus, editor_container, None, None);
        if app.config.editor.edit_mode_highlight {
            super::overlay_markdown_highlight(frame, app, editor_container);
        }
        super::overlay_text_alignment(frame, app, editor_container);
        super::overlay_zen_focus(frame, app, editor_container);
        // Scrollbar for editor body
        if app.config.ui.scrollbars && !(app.zen_mode && app.config.editor.zen_hide_scrollbar) {
            let content_len = app.editor.body.lines().len();
            // The editor block uses Padding::new(0, 2, 1, 0), so the textarea
            // renders height-1 rows. Match refresh_textarea_viewport
            // (src/ui/mod.rs:1516), which derives visible rows from
            // block.inner(area).
            let viewport_len = editor_container.height.saturating_sub(1) as usize;
            let area = editor_container;
            let meta = crate::ui::scrollbar::ScrollbarMeta {
                track: crate::ui::scrollbar::track_rect(area),
                content_len,
                viewport_len,
            };
            app.editor.last_scroll = Some(meta);
            crate::ui::scrollbar::draw_scrollbar(
                frame,
                area,
                content_len,
                viewport_len,
                app.editor.body_viewport_row as usize,
                content_len.saturating_sub(viewport_len),
                &app.app_theme,
            );
        } else {
            app.editor.last_scroll = None;
        }
    }

    // Sync preview scroll with editor scroll. Skip while the user is
    // mouse-scrolling the preview independently: re-sync once the editor
    // viewport moves or the preview content is rebuilt.
    if let Some(renderer) = &mut app.editor.md_preview_renderer
        && renderer.document().is_some()
    {
        if let Some(preview_area) = preview_area_rect {
            let block = Block::default()
                .style(app.app_theme.preview_bg_style())
                .borders(Borders::NONE)
                .padding(Padding::new(2, 2, 1, 1));
            let inner = block.inner(preview_area);

            let anchor = app
                .editor
                .visual_row_cache
                .source_position(usize::from(app.editor.body_viewport_row));
            let last_synced = app.editor.preview_scroll_synced_anchor;
            let resync = !app.editor.preview_scroll_overridden || last_synced != Some(anchor);
            if resync {
                // Editor and preview have different wrapping and block heights.
                // Synchronize through source positions, never raw screen-row offsets.
                let rendered_start = renderer.source_to_rendered_position(anchor.0, anchor.1);
                renderer.set_scroll_offset(rendered_start, usize::from(inner.height));
                app.editor.preview_scroll_synced_anchor = Some(anchor);
                app.editor.preview_scroll_overridden = false;
            }
        }
    }

    if let Some(preview_area_rect) = preview_area_rect {
        app.editor.markdown_inner_rect = None;
        if let Some(renderer) = &app.editor.md_preview_renderer {
            if let Some(doc) = renderer.document() {
                let block = Block::default()
                    .style(app.app_theme.preview_bg_style())
                    .borders(Borders::NONE)
                    .padding(Padding::new(0, 2, 1, 1));
                let inner = block.inner(preview_area_rect);
                frame.render_widget(block, preview_area_rect);
                app.editor.markdown_inner_rect = Some(inner);

                let scroll = renderer.scroll_offset();
                let widget_range = scroll..(scroll + inner.height as usize);
                let widget = crate::markdown::MarkdownWidget::new(doc, widget_range.clone())
                    .text_align(app.editor.text_align);
                frame.render_widget(widget, inner);

                // Overlay images continuously
                if let (Some(_), Some(decode_tx)) =
                    (&app.editor.image_picker, &app.editor.image_decode_tx)
                {
                    let col_width = inner.width;
                    for (local_line_idx, url) in doc.image_slots(widget_range) {
                        let resolved = app.storage.resolve_attachment(url);
                        let path = resolved.unwrap_or_else(|| app.storage.notes_dir.join(url));
                        if !path.exists() {
                            continue;
                        }
                        let key = path;
                        if app.editor.image_cache.get_proto(&key).is_none() {
                            app.editor.image_cache.request(key.clone(), 2048, decode_tx);
                        }

                        if let Some(proto) = app.editor.image_cache.get_proto(&key) {
                            let row = inner.y + local_line_idx as u16;
                            let max_h = app.config.image.preview_rows as u16;
                            let img_rect = Rect::new(
                                inner.x,
                                row,
                                col_width.min(inner.width),
                                max_h.min(inner.bottom().saturating_sub(row)),
                            );
                            if img_rect.width > 1 && img_rect.height > 1 {
                                frame.render_widget(ratatui::widgets::Clear, img_rect);
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
            } else {
                let loading = Paragraph::new("Rendering preview...")
                    .style(Style::default().fg(app.app_theme.muted))
                    .block(
                        Block::default()
                            .style(app.app_theme.preview_bg_style())
                            .borders(Borders::NONE)
                            .padding(Padding::new(2, 2, 1, 1)),
                    );
                frame.render_widget(loading, preview_area_rect);
            }
        }
    }
    if app.zen_mode {
        let blank = Paragraph::new("").style(app.app_theme.bg_style());
        frame.render_widget(blank, hint_area);
    } else {
        let kb = &app.keybinds;
        let hints_items = vec![
            (kb.display_edit(EditAction::CycleFocus), "focus"),
            (
                kb.display_edit(EditAction::ToggleMarkdownPreview),
                "preview",
            ),
            (kb.display_edit(EditAction::ToggleOutline), "outline"),
            (kb.display_edit(EditAction::ToggleLinks), "links"),
            (kb.display_edit(EditAction::ToggleFileTree), "files"),
            (kb.display_edit(EditAction::NewTab), "new-tab"),
            (kb.display_edit(EditAction::CloseTab), "close-tab"),
            (kb.display_edit(EditAction::Find), "find"),
            (kb.edit_keys_display(EditAction::Back), "back"),
            ("F1".to_string(), "help"),
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
    }
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
    if app.editor.link_complete.is_some() && !app.preview_fullscreen {
        draw_link_completion(frame, editor_container, app);
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
/// Tab strip for the merged edit-view header: the selected tab shows the
/// note title (live while editing) and the only save indicator, so the old
/// separate title row and its duplicate `⨯`/`✓` are gone. While title focus
/// is active the selected tab expands into the editable title field.
/// Records tab rects for mouse hit-testing; the active-tab rect doubles as
/// `header_title_rect` (title click-to-focus).
fn draw_tab_strip(
    frame: &mut Frame,
    strip: Rect,
    editor: &mut crate::editor::NoteEditor,
    theme: &crate::app_theme::AppThemeColors,
    focus: EditFocus,
) {
    use crate::events::get_title_text;
    editor.tab_bar_rects.clear();
    editor.tab_close_rects.clear();
    editor.new_tab_rect = Rect::default();

    if strip.width < 10 || editor.tabs.is_empty() {
        editor.tab_bar_rect = Rect::default();
        editor.header_title_rect = Rect::default();
        return;
    }

    let active = editor.active_tab.min(editor.tabs.len() - 1);
    let title_focused = focus == EditFocus::Title;
    let live_title = get_title_text(&editor.title_editor).into_owned();
    let mut x = strip.x;
    let end = strip.right();
    // Reserve 4 cols for the `+` button.
    for (i, tab) in editor.tabs.iter().enumerate() {
        let is_active = i == active;
        let mut label = if is_active && !live_title.trim().is_empty() {
            live_title.clone()
        } else if tab.title_snapshot.trim().is_empty() {
            "Untitled".to_string()
        } else {
            tab.title_snapshot.clone()
        };
        if label.chars().count() > 14 {
            label = format!("{}…", label.chars().take(13).collect::<String>());
        }
        // Single save indicator: the active tab only. The old separate
        // title row (and its duplicate ⨯/✓) no longer exists.
        let dirty =
            is_active && !matches!(editor.autosave_status, crate::editor::AutosaveStatus::Saved);
        let marker = if dirty { "●" } else { " " };
        // Trailing `×` closes the tab with one click (or middle-click).
        let text = format!(" {label}{marker} × ");
        let natural_w = text.width() as u16;
        let avail = end.saturating_sub(x);
        // The edited title takes the remaining strip while focused.
        let mut w = natural_w.min(avail);
        if is_active && title_focused {
            w = avail.saturating_sub(4).max(w);
        }
        if w < 6 {
            break;
        }
        let rect = Rect::new(x, strip.y, w, 1);
        let style = if is_active {
            Style::default().fg(theme.highlight_fg).bg(theme.accent)
        } else {
            Style::default().fg(theme.muted)
        };
        if is_active && title_focused {
            // Leave the trailing `● × ` suffix visible so the save state
            // and the close affordance match unfocused tabs.
            let suffix = format!("{marker} × ");
            let suffix_w = suffix.width() as u16;
            let edit_rect = Rect::new(rect.x, rect.y, rect.width.saturating_sub(suffix_w), 1);
            editor
                .title_editor
                .set_style(Style::default().fg(theme.highlight_fg).bg(theme.accent));
            editor.title_editor.set_block(
                Block::default()
                    .style(Style::default().bg(theme.accent))
                    .borders(Borders::NONE),
            );
            editor
                .title_editor
                .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            editor.title_editor.set_cursor_line_style(Style::default());
            frame.render_widget(&editor.title_editor, edit_rect);
            frame.render_widget(
                Paragraph::new(suffix).style(style),
                Rect::new(edit_rect.right(), rect.y, suffix_w, 1),
            );
            let (r, c) = crate::ui::refresh_textarea_viewport(
                &editor.title_editor,
                editor.title_viewport_row,
                editor.title_viewport_col,
                edit_rect,
                false,
            );
            editor.title_viewport_row = r;
            editor.title_viewport_col = c;
        } else {
            frame.render_widget(Paragraph::new(text).style(style), rect);
        }
        editor.tab_bar_rects.push(rect);
        editor
            .tab_close_rects
            .push(Rect::new(rect.right().saturating_sub(3), strip.y, 3, 1));
        if is_active {
            editor.header_title_rect = rect;
        }
        x += w;
        if x >= end.saturating_sub(5) {
            break;
        }
    }
    // Clicks on the status segments fall through (outside the tab strip).
    editor.tab_bar_rect = Rect::new(strip.x, strip.y, x.saturating_sub(strip.x), 1);
    if end.saturating_sub(x) >= 4 {
        let rect = Rect::new(end - 4, strip.y, 4, 1);
        frame.render_widget(
            Paragraph::new(" + ").style(Style::default().fg(theme.accent)),
            rect,
        );
        editor.new_tab_rect = rect;
        editor.tab_bar_rect = editor.tab_bar_rect.union(rect);
    } else {
        editor.new_tab_rect = Rect::default();
    }
}

fn draw_file_tree_pane(frame: &mut Frame, area: Rect, app: &mut App, focus: EditFocus) {
    let theme = &app.app_theme;
    frame.render_widget(Block::default().style(theme.preview_bg_style()), area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);
    let title_style = if focus == EditFocus::FileTree {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.heading)
            .add_modifier(Modifier::BOLD)
    };
    // Reuse the notes list tree verbatim: same rows, icons, counts and
    // density as the toggle-notes-layout tree view.
    let len = app.list.visual_list.len();
    frame.render_widget(
        Paragraph::new(format!("  FILES ({len})")).style(title_style),
        chunks[1],
    );
    if len == 0 {
        frame.render_widget(
            Paragraph::new("  No notes").style(Style::default().fg(theme.muted)),
            chunks[3],
        );
        app.editor.file_tree_list_rect = Rect::default();
        return;
    }
    let list_items: Vec<ListItem> = (0..len).map(|i| app.format_visual_item(i)).collect();
    let pitch = crate::app::tabs::tree_row_pitch(app).max(1) as usize;
    let viewport_items = (chunks[3].height as usize / pitch).max(1);
    // Keep selection in view.
    let sel = app.editor.file_tree_selected.min(len - 1);
    app.editor.file_tree_selected = sel;
    if sel < app.editor.file_tree_scroll {
        app.editor.file_tree_scroll = sel;
    } else if sel >= app.editor.file_tree_scroll + viewport_items {
        app.editor.file_tree_scroll = sel + 1 - viewport_items;
    }
    let max_off = len.saturating_sub(viewport_items);
    app.editor.file_tree_scroll = app.editor.file_tree_scroll.min(max_off);
    let mut state = crate::ui::list_state_selected(Some(sel), app.editor.file_tree_scroll);
    let list_area = Rect::new(chunks[3].x, chunks[3].y, chunks[3].width, chunks[3].height);
    let list = List::new(list_items).highlight_style(
        Style::default()
            .bg(theme.highlight_bg)
            .fg(theme.highlight_fg),
    );
    frame.render_stateful_widget(list, list_area, &mut state);
    app.editor.file_tree_scroll = state.offset();
    app.editor.file_tree_list_rect = list_area;
    crate::ui::paint_list_hover(
        frame,
        list_area,
        &state,
        len,
        app.mouse_pos,
        theme.hover_style(),
    );
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
    renderer.set_page_height(inner.height as usize);
    renderer.set_viewport(0, inner.height as usize);
    if let Some(doc) = renderer.document() {
        let padding_block = Block::default()
            .style(app.app_theme.preview_bg_style())
            .borders(Borders::NONE)
            .padding(Padding::new(1, 1, 0, 0));
        let padded_inner = padding_block.inner(inner);
        frame.render_widget(padding_block, inner);

        let range = renderer.current_page_range();
        let widget =
            crate::markdown::MarkdownWidget::new(doc, range).text_align(app.editor.text_align);
        frame.render_widget(widget, padded_inner);
    } else {
        let p = Paragraph::new("Loading…").style(Style::default().fg(app.app_theme.muted));
        frame.render_widget(p, inner);
    }
}
