use ratatui::{prelude::*, widgets::*};
use crate::app::{App, VIRTUAL_PINNED_PATH, VIRTUAL_PINNED_LABEL, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::constants::LIST_HELP_HINTS;
use super::{
    PopupSize, PreviewHeaderInfo, build_tab_spans, draw_view_title_bar,
    draw_view_title_bar_with_tabs, draw_status_bar, draw_dim_vline,
    draw_corner_watermark, draw_popup_frame, draw_confirm_popup,
    draw_template_popup, format_relative_time, build_list_widget,
    resolved_status_hint, ext_badge, popup_block
};

const GRID_TILE_W: u16 = 10; // outer width incl. border
const GRID_TILE_H: u16 = 5; // outer height incl. border
const GRID_GAP: u16 = 1; // space between tiles (h and v)
const GRID_LEFT_MARGIN: u16 = 2; // left inset inside list_area
const GRID_TOP_MARGIN: u16 = 3; // top inset inside list_area

pub fn draw_list_view(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);
    if app.preview_fullscreen {
        let preview_info = get_preview_info(app);
        draw_view_title_bar(frame, chunks[0], "Notes", &app.app_theme, preview_info);
    } else if app.list.notes_layout == crate::config::NotesLayout::Grid {
        let tabs = [("Vault", Some("\u{f07b}")), ("Pinned", Some("\u{f4cc}"))];
        let is_pinned = app.list.grid_folder == VIRTUAL_PINNED_PATH;
        let tab_spans = build_tab_spans(
            &tabs,
            if is_pinned { 1 } else { 0 },
            &app.app_theme,
            app.config.ui.tab_icons_only,
        );
        draw_view_title_bar_with_tabs(frame, chunks[0], "Notes", tab_spans, &app.app_theme);

        // Show details of the selected note at the top right (clock/relative time + tags)
        if let Some(crate::app::VisualItem::Note { summary_idx, .. }) =
            app.list.visual_list.get(app.list.visual_index)
        {
            let s = &app.notes[*summary_idx];
            let mut spans = Vec::new();

            let when = format_relative_time(s.updated_at);
            spans.push(Span::styled(
                " \u{f017} ",
                Style::default().fg(app.app_theme.muted),
            ));
            spans.push(Span::styled(
                when.into_owned(),
                Style::default().fg(app.app_theme.muted),
            ));

            if !s.tags.is_empty() {
                spans.push(Span::styled(
                    "  \u{f02b} ",
                    Style::default()
                        .fg(app.app_theme.tag)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    s.tags.join(", "),
                    Style::default().fg(app.app_theme.fg),
                ));
            }
            spans.push(Span::raw(" ")); // padding right

            let detail_para = Paragraph::new(Line::from(spans)).alignment(Alignment::Right);
            frame.render_widget(detail_para, chunks[0]);
        } else if let Some(crate::app::VisualItem::Folder {
            name, note_count, ..
        }) = app.list.visual_list.get(app.list.visual_index)
            && name != ".."
        {
            let mut spans = Vec::new();
            let suffix = if *note_count == 1 { "note" } else { "notes" };
            spans.push(Span::styled(
                " \u{f0ca} ",
                Style::default().fg(app.app_theme.folder),
            ));
            spans.push(Span::styled(
                format!("{note_count} {suffix}"),
                Style::default().fg(app.app_theme.fg),
            ));
            spans.push(Span::raw(" ")); // padding right

            let detail_para = Paragraph::new(Line::from(spans)).alignment(Alignment::Right);
            frame.render_widget(detail_para, chunks[0]);
        }
    } else {
        draw_view_title_bar(frame, chunks[0], "Notes", &app.app_theme, None);
    }

    let (list_area, preview_area, calendar_area) = list_view_layout(
        area,
        app.list.preview_enabled,
        app.preview_position,
        app.list.calendar_enabled,
        app.preview_fullscreen,
    );
    if let Some(p) = preview_area {
        app.list.last_preview_pane_width = p.width;
    }

    if !app.preview_fullscreen {
        let is_grid = app.list.notes_layout == crate::config::NotesLayout::Grid;
        let mut items: Vec<ListItem> = Vec::new();

        if is_grid {
            app.list.grid_tiles.clear();

            // --- render directory breadcrumbs at the top of the list area ---
            let is_pinned = app.list.grid_folder == VIRTUAL_PINNED_PATH;
            let mut spans = Vec::new();
            if is_pinned {
                spans.push(Span::styled(
                    " \u{f4cc} Pinned",
                    Style::default()
                        .fg(app.app_theme.heading)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    " \u{f07b} Vault",
                    Style::default()
                        .fg(app.app_theme.folder)
                        .add_modifier(Modifier::BOLD),
                ));
                if !app.list.grid_folder.is_empty() {
                    for part in app.list.grid_folder.split('/') {
                        spans.push(Span::styled(
                            " / ",
                            Style::default().fg(app.app_theme.muted),
                        ));
                        spans.push(Span::styled(
                            part.to_string(),
                            Style::default().fg(app.app_theme.fg),
                        ));
                    }
                }
            }
            let dir_rect = Rect::new(list_area.x, list_area.y + 1, list_area.width, 1);
            frame.render_widget(Paragraph::new(Line::from(spans)), dir_rect);

            // --- columns / visible rows ---
            let cols = ((list_area.width.saturating_sub(GRID_LEFT_MARGIN + GRID_GAP))
                / (GRID_TILE_W + GRID_GAP))
                .max(1) as usize;
            let rows = ((list_area.height.saturating_sub(GRID_TOP_MARGIN + GRID_GAP))
                / (GRID_TILE_H + GRID_GAP)) as usize;
            app.list.grid_columns = cols; // events.rs grid nav reads this (Up/Down move by cols)

            let len = app.list.visual_list.len();

            // --- clamp grid_scroll so the selected tile stays visible, without over-scrolling ---
            if cols > 0 && rows > 0 && len > 0 {
                let sel_row = app.list.visual_index / cols;
                if sel_row < app.list.grid_scroll {
                    app.list.grid_scroll = sel_row;
                }
                let last_visible = app.list.grid_scroll + rows.saturating_sub(1);
                if sel_row > last_visible {
                    app.list.grid_scroll = sel_row.saturating_sub(rows.saturating_sub(1));
                }
                let max_scroll = (len - 1) / cols;
                if app.list.grid_scroll > max_scroll {
                    app.list.grid_scroll = max_scroll;
                }
            } else {
                app.list.grid_scroll = 0;
            }

            let start = app.list.grid_scroll * cols;
            let count = (rows * cols).min(len.saturating_sub(start));
            let buf = frame.buffer_mut();

            for i in 0..count {
                let vi = start + i;
                if vi >= len {
                    break;
                }
                let row = i / cols;
                let col = i % cols;
                let tile_rect = ratatui::layout::Rect::new(
                    list_area.x + GRID_LEFT_MARGIN + (col as u16) * (GRID_TILE_W + GRID_GAP),
                    list_area.y + GRID_TOP_MARGIN + (row as u16) * (GRID_TILE_H + GRID_GAP),
                    GRID_TILE_W,
                    GRID_TILE_H,
                );
                let is_selected = vi == app.list.visual_index;

                // --- resolve (icon char, glyph color, display name): SAME mapping the old code used ---
                let item = &app.list.visual_list[vi];
                let (icon_char, glyph_color, raw_name) = match item {
                    crate::app::VisualItem::Folder { name, .. } => {
                        let is_pinned = name == VIRTUAL_PINNED_LABEL;
                        let is_parent = name == "..";
                        let ic = if is_pinned {
                            '\u{f4cc}'
                        } else if is_parent {
                            '\u{f062}' // Arrow Up ()
                        } else {
                            '\u{f07b}' // Folder ()
                        };
                        let col = if is_pinned {
                            app.app_theme.heading
                        } else {
                            app.app_theme.folder
                        };
                        (ic, col, name.clone())
                    }
                    crate::app::VisualItem::Note {
                        summary_idx,
                        is_clin,
                        is_draw,
                        is_canvas,
                        ..
                    } => {
                        let s = &app.notes[*summary_idx];
                        let col = if s.pinned {
                            app.app_theme.heading
                        } else if *is_clin {
                            app.app_theme.destructive
                        } else if *is_draw {
                            app.app_theme.success
                        } else if *is_canvas {
                            app.app_theme.accent
                        } else {
                            app.app_theme.text
                        };
                        let ic = if s.pinned {
                            '\u{f4cc}'
                        } else if *is_clin {
                            '\u{f023}'
                        } else if *is_draw {
                            '\u{f1fc}'
                        } else if *is_canvas {
                            '\u{f005}'
                        } else {
                            '\u{f15c}'
                        };
                        (ic, col, s.title.clone())
                    }
                    crate::app::VisualItem::CreateNew { .. } => {
                        ('\u{f067}', app.app_theme.success, "Create...".to_string())
                    }
                };

                // --- tile border (plain border = "button") ---
                let mut block = Block::default().borders(Borders::ALL);
                if is_selected {
                    block = block.border_style(Style::default().fg(app.app_theme.highlight_bg));
                } else {
                    block = block.border_style(Style::default().fg(app.app_theme.border));
                }
                let inner = block.inner(tile_rect);
                block.render(tile_rect, buf); // paints border

                // --- icon: centered on the top inner row ---
                let icon_style = if is_selected {
                    Style::default()
                        .fg(glyph_color)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(glyph_color)
                };
                let inner_w = inner.width as usize; // GRID_TILE_W - 2 = 10
                let icon_x = inner.x + (inner_w.saturating_sub(1) / 2) as u16; // center the 1-wide glyph
                if let Some(cell) = buf.cell_mut((icon_x, inner.y)) {
                    cell.set_char(icon_char).set_style(icon_style);
                }

                // --- tag icon: top right corner for items that have tags ---
                let has_tags = match item {
                    crate::app::VisualItem::Note { summary_idx, .. } => {
                        !app.notes[*summary_idx].tags.is_empty()
                    }
                    _ => false,
                };
                if has_tags {
                    let tag_x = inner.x + inner.width.saturating_sub(1);
                    if let Some(cell) = buf.cell_mut((tag_x, inner.y)) {
                        let tag_style = if is_selected {
                            Style::default()
                                .fg(app.app_theme.tag)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(app.app_theme.tag)
                        };
                        cell.set_char('\u{f02b}').set_style(tag_style);
                    }
                }

                // --- name: sanitize, truncate to inner width, center, write on the bottom row (row 2) ---
                let sanitized = crate::sanitize::sanitize_for_terminal(&raw_name);
                let mut chars: Vec<char> = sanitized.chars().collect();
                if chars.len() > inner_w {
                    chars.truncate(inner_w - 1);
                    chars.push('…');
                }
                let pad = inner_w.saturating_sub(chars.len());
                let left = pad / 2;
                let name_style = if is_selected {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let mut name_string: String = " ".repeat(left);
                name_string.extend(chars.iter());
                let name_row = inner.y + 2; // bottom row of the 3 inner rows
                for (k, ch) in name_string.chars().enumerate() {
                    if let Some(cell) = buf.cell_mut((inner.x + k as u16, name_row)) {
                        cell.set_char(ch).set_style(name_style);
                    }
                }

                // --- record tile for mouse hit-testing ---
                app.list.grid_tiles.push(crate::list_view::GridTile {
                    visual_index: vi,
                    rect: tile_rect,
                });
            }
            // do NOT render a List widget here; do NOT touch list_state (tree view still uses it).
        } else {
            items.reserve(app.list.display_items.len());
            for item in &app.list.display_items {
                items.push(item.clone());
            }
            let list = List::new(items)
                .block(
                    Block::default()
                        .style(app.app_theme.bg_style())
                        .borders(Borders::NONE)
                        .padding(Padding::new(2, 2, 1, 1)),
                )
                .highlight_style(
                    Style::default()
                        .fg(app.app_theme.highlight_fg)
                        .bg(app.app_theme.highlight_bg)
                        .add_modifier(Modifier::BOLD),
                );

            app.list.list_state.select(Some(app.list.visual_index));
            frame.render_stateful_widget(list, list_area, &mut app.list.list_state);
        }

        if app.list.list_mode == crate::list_view::ListMode::Select {
            let mode_label = if app.list.tag_to_assign.is_some() {
                "TAG MODE"
            } else {
                "SELECT MODE"
            };
            let select_hint = format!(
                " {}: {} selected ",
                mode_label,
                app.list.selected_indices.len()
            );
            let select_para = Paragraph::new(Span::styled(
                select_hint,
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
            let max_width = list_area.width.saturating_sub(4);
            let select_width = 34.min(max_width);
            let select_area = Rect::new(list_area.x + 2, list_area.y, select_width, 1);
            frame.render_widget(select_para, select_area);
        }
    }
    if let Some(preview_rect) = preview_area {
        let hide_encrypted = app.preview_encryption
            && app
                .list
                .visual_list
                .get(app.list.visual_index)
                .is_some_and(|item| {
                    matches!(item, crate::app::VisualItem::Note { is_clin: true, .. })
                });

        let content_is_current = app.list.preview_content_index == Some(app.list.visual_index);
        let content = if content_is_current || app.list.pending_preview_update {
            app.list.preview_content.as_ref()
        } else {
            None
        };

        crate::preview::draw_preview_pane(
            frame,
            preview_rect,
            &app.app_theme,
            content,
            hide_encrypted,
            app.list.snapshot_scroll_offset,
        );
    }
    if let Some(cal_rect) = calendar_area {
        if app.config.goals.enabled && cal_rect.width >= 42 {
            let layout_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(25), Constraint::Min(0)])
                .split(cal_rect);
            crate::calendar::draw_calendar(frame, layout_chunks[0], &app.app_theme, &app.notes);
            let _ = app.get_current_goals_progress();
            crate::goals::draw_goals_progress(
                frame,
                layout_chunks[1],
                &app.app_theme,
                &app.goals_progress,
                &app.config.goals,
            );
        } else {
            crate::calendar::draw_calendar(frame, cal_rect, &app.app_theme, &app.notes);
        }
    }

    let hint = resolved_status_hint(app, LIST_HELP_HINTS);
    let badge = Some(ext_badge(
        app.editor.external_editor_enabled,
        &app.app_theme,
    ));
    draw_status_bar(frame, chunks[2], &app.app_theme, badge, &hint, None);
    draw_corner_watermark(frame, chunks[2], app.app_theme.muted);
    if app.list.preview_enabled && !app.preview_fullscreen {
        let constraints = match app.preview_position {
            crate::config::PreviewPosition::Left => [
                Constraint::Ratio(43, 100),
                Constraint::Length(1),
                Constraint::Min(0),
            ],
            crate::config::PreviewPosition::Right => [
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Ratio(43, 100),
            ],
        };
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(chunks[1]);
        draw_dim_vline(frame, full_cols[1], app.app_theme.muted);
    }

    if let Some(popup) = &app.popups.template {
        draw_template_popup(frame, popup, area, &app.app_theme);
    }

    if let Some(popup) = &mut app.popups.folder {
        let title = match popup.mode {
            crate::popups::FolderPopupMode::Create { .. } => "NEW FOLDER",
            crate::popups::FolderPopupMode::Rename { .. } => "RENAME FOLDER",
        };
        let content = draw_popup_frame(
            frame,
            area,
            title,
            PopupSize::Prompt,
            "Enter confirm · Esc cancel",
            &app.app_theme,
        );

        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.heading)),
        );
        frame.render_widget(&popup.input, content);
    }

    if let Some(popup) = &mut app.popups.tag {
        let suggestion_height = if popup.suggestions.is_empty() {
            0u16
        } else {
            (popup.suggestions.len() as u16).clamp(1, 5)
        };
        let content = draw_popup_frame(
            frame,
            area,
            "TAGS",
            PopupSize::Large,
            "Ctrl+S batch assign · Tab accept · Enter save · d delete from all · Esc cancel",
            &app.app_theme,
        );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3u16 + suggestion_height),
                Constraint::Min(3),
            ])
            .split(content);

        let input_border = if popup.focus == crate::popups::TagPopupFocus::Input {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        let input_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(chunks[0]);

        let input_block = Block::default()
            .style(app.app_theme.bg_style())
            .borders(Borders::ALL)
            .border_style(input_border)
            .title("");
        let input_inner = input_block.inner(input_chunks[0]);
        frame.render_widget(input_block, chunks[0]);
        frame.render_widget(&popup.input, input_inner);

        if !popup.suggestions.is_empty() {
            let suggestion_items: Vec<ListItem> = popup
                .suggestions
                .iter()
                .enumerate()
                .map(|(i, tag)| {
                    let style = if i == popup.suggestion_index {
                        Style::default()
                            .fg(app.app_theme.highlight_fg)
                            .bg(app.app_theme.heading)
                    } else {
                        Style::default()
                    };
                    ListItem::new(format!("  {tag}")).style(style)
                })
                .collect();

            let suggestions_list = List::new(suggestion_items)
                .block(
                    Block::default()
                        .borders(Borders::NONE)
                        .style(app.app_theme.bg_style()),
                )
                .highlight_style(Style::default());

            frame.render_widget(suggestions_list, input_chunks[1]);
        }

        let all_tags_border = if popup.focus == crate::popups::TagPopupFocus::AllTagsList {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        let tag_empty = popup.all_tags.is_empty();
        let tag_items: Vec<ListItem> = if tag_empty {
            vec![ListItem::new(Span::styled(
                "No tags found",
                Style::default().fg(app.app_theme.muted),
            ))]
        } else {
            popup
                .all_tags
                .iter()
                .map(|tag| ListItem::new(tag.to_string()))
                .collect()
        };

        let tags_list = build_list_widget(tag_items, &app.app_theme)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(all_tags_border),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");

        let mut tags_state = ListState::default();
        if popup.focus == crate::popups::TagPopupFocus::AllTagsList && !popup.all_tags.is_empty() {
            tags_state.select(Some(popup.all_tags_selected));
        }
        frame.render_stateful_widget(tags_list, chunks[1], &mut tags_state);
    }

    if let Some(picker) = &mut app.popups.folder_picker {
        let title = match picker.mode {
            crate::popups::FolderPickerMode::CopyNote { .. } => "COPY",
            _ => "MOVE",
        };
        let content = draw_popup_frame(
            frame,
            area,
            title,
            PopupSize::Large,
            "Tab switch  Enter move  Esc cancel",
            &app.app_theme,
        );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(content);

        let search_border = if picker.focus == crate::app::FolderPickerFocus::Search {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        picker.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(search_border)
                .title(""),
        );
        frame.render_widget(&picker.input, chunks[0]);

        let items: Vec<ListItem> = if picker.filtered_folders.is_empty() {
            vec![ListItem::new(Span::styled(
                "(no matching folders)",
                Style::default().fg(app.app_theme.muted),
            ))]
        } else {
            picker
                .filtered_folders
                .iter()
                .map(|f| {
                    let label = if f.is_empty() { "Vault (Root)" } else { f };
                    ListItem::new(label)
                })
                .collect()
        };

        let results_border = if picker.focus == crate::app::FolderPickerFocus::Results {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };
        let list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(results_border)
                    .title(""),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");

        let mut state = ListState::default();
        if picker.focus == crate::app::FolderPickerFocus::Results
            && !picker.filtered_folders.is_empty()
        {
            state.select(Some(picker.selected));
        }

        frame.render_stateful_widget(list, chunks[1], &mut state);
    }

    if let Some(palette) = &mut app.command_palette {
        let content = draw_popup_frame(
            frame,
            area,
            "COMMANDS",
            PopupSize::Large,
            "Tab category · Enter run · ↑/↓ select · Esc close",
            &app.app_theme,
        );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // search input
                Constraint::Length(1), // tab bar
                Constraint::Min(0),    // results list
            ])
            .split(content);

        palette.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.muted))
                .title(""),
        );
        frame.render_widget(&palette.input, chunks[0]);

        let tabs: Vec<(&str, Option<&str>)> = crate::palette::PALETTE_TABS
            .iter()
            .map(|(l, g, _)| (*l, Some(*g)))
            .collect();
        let tab_spans = build_tab_spans(
            &tabs,
            palette.active_tab,
            &app.app_theme,
            app.config.ui.tab_icons_only,
        );
        let tabs_w = Paragraph::new(Line::from(tab_spans))
            .alignment(Alignment::Center)
            .style(app.app_theme.hint_line_bg_style());
        frame.render_widget(tabs_w, chunks[1]);

        let items: Vec<ListItem> = palette
            .items
            .iter()
            .map(|item| {
                let mut spans = vec![Span::styled(
                    format!("{} ", &item.glyph),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                )];
                spans.extend(crate::ui::style_palette_name(&item.name, &app.app_theme));
                ListItem::new(vec![
                    Line::from(spans),
                    Line::from(Span::styled(
                        &item.description,
                        Style::default().fg(app.app_theme.muted),
                    )),
                ])
            })
            .collect();

        let list = ratatui::widgets::List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(app.app_theme.muted))
                    .title(""),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");

        frame.render_stateful_widget(list, chunks[2], &mut palette.state);
    }

    if let Some(popup) = &mut app.popups.note_rename {
        let content = draw_popup_frame(
            frame,
            area,
            "RENAME",
            PopupSize::Prompt,
            "Enter rename · Esc cancel",
            &app.app_theme,
        );

        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.heading)),
        );
        frame.render_widget(&popup.input, content);
    }

    if let Some(popup) = &mut app.popups.goals {
        let (title, sub) = match popup.mode {
            crate::popups::GoalsPopupMode::WordGoal => {
                ("DAILY WORD GOAL", "Enter word count · Esc cancel")
            }
            crate::popups::GoalsPopupMode::NoteGoal => {
                ("DAILY NOTE GOAL", "Enter note count · Esc cancel")
            }
        };
        let content = draw_popup_frame(frame, area, title, PopupSize::Prompt, sub, &app.app_theme);

        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.heading)),
        );
        frame.render_widget(&popup.input, content);
    }

    if let Some((popup, format)) = &mut app.popups.create_note {
        let title = match format {
            crate::popups::NoteFormat::Markdown => "NEW NOTE",
            crate::popups::NoteFormat::Draw => "NEW DRAWING",
            crate::popups::NoteFormat::Canvas => "NEW CANVAS",
            crate::popups::NoteFormat::PlainText => "NEW TEXT FILE",
        };
        let content = draw_popup_frame(
            frame,
            area,
            title,
            PopupSize::Prompt,
            "Enter create · Esc cancel",
            &app.app_theme,
        );
        popup.input.set_block(popup_block("", &app.app_theme));
        frame.render_widget(&popup.input, content);
    }

    if let Some(popup) = &mut app.popups.import {
        let title = match popup.source {
            crate::popups::ImportSource::File => "IMPORT FILE",
            crate::popups::ImportSource::Csv => "IMPORT CSV/TSV",
            crate::popups::ImportSource::Json => "IMPORT JSON",
            crate::popups::ImportSource::Url => "IMPORT URL",
            crate::popups::ImportSource::Clipboard => "IMPORT CLIPBOARD",
        };
        let content = draw_popup_frame(
            frame,
            area,
            title,
            PopupSize::Large,
            "Enter import · Esc cancel",
            &app.app_theme,
        );
        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.muted)),
        );
        frame.render_widget(&popup.input, content);
    }

    if let Some(popup) = &mut app.popups.search {
        let content = draw_popup_frame(
            frame,
            area,
            "SEARCH",
            PopupSize::Large,
            "Tab switch · Enter open · Esc cancel · f:folder p:pinned t:tag g:text · \\e\\ escapes filters",
            &app.app_theme,
        );

        let query_text = popup.input.lines().join("");
        let parsed = crate::app::parse_search_query(&query_text);
        let has_filter = parsed.folder_filter.is_some()
            || parsed.pinned_only
            || parsed.tag_filter.is_some()
            || parsed.grep_mode;

        let constraints = if has_filter {
            vec![
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(3),
            ]
        } else {
            vec![Constraint::Length(3), Constraint::Min(3)]
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(content);

        if has_filter {
            let mut spans: Vec<Span<'static>> = vec![Span::raw("  ")];
            let mut first = true;

            let add_sep = |spans: &mut Vec<Span<'static>>,
                           first: &mut bool,
                           theme: &AppThemeColors| {
                if !*first {
                    spans.push(Span::styled(
                        " · ",
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                *first = false;
            };

            if let Some(f) = &parsed.folder_filter {
                let text = if f.is_empty() { "Vault" } else { f.as_str() };
                add_sep(&mut spans, &mut first, &app.app_theme);
                spans.push(Span::styled(
                    "\u{f07c} ",
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if parsed.pinned_only {
                add_sep(&mut spans, &mut first, &app.app_theme);
                spans.push(Span::styled(
                    "\u{f08d} Pinned",
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if parsed.grep_mode {
                add_sep(&mut spans, &mut first, &app.app_theme);
                let grep_display = if parsed.grep_text.is_empty() {
                    "Grep".to_string()
                } else {
                    parsed.grep_text.clone()
                };
                spans.push(Span::styled(
                    format!("\u{f002} {grep_display}"),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if let Some(tags) = &parsed.tag_filter {
                add_sep(&mut spans, &mut first, &app.app_theme);
                let tag_text = if tags.is_empty() {
                    String::new()
                } else {
                    tags.join(", ")
                };
                spans.push(Span::styled(
                    "\u{f02b} ",
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    tag_text,
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            let filter_line = Line::from(spans);
            let filter_para = Paragraph::new(filter_line).style(app.app_theme.bg_style());
            frame.render_widget(filter_para, chunks[1]);
        }

        let input_chunk = chunks[0];
        let results_chunk = if has_filter { chunks[2] } else { chunks[1] };

        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(if popup.focus == crate::popups::SearchFocus::Input {
                    Style::default().fg(app.app_theme.heading)
                } else {
                    Style::default().fg(app.app_theme.muted)
                })
                .title(""),
        );
        frame.render_widget(&popup.input, input_chunk);

        let has_title = !popup.title_results.is_empty();
        let has_grep = !popup.grep_results.is_empty();

        let results_focused = popup.focus == crate::popups::SearchFocus::Results;
        let results_border = if results_focused {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };

        let (all_items, results_title) = if has_grep {
            let mut visible: Vec<(usize, String)> = Vec::new();
            let mut i = 0;
            while i < popup.grep_results.len() {
                let is_collapsed = popup.grep_is_header[i] && !popup.grep_expanded.contains(&i);
                let icon = if popup.grep_is_header[i] {
                    if is_collapsed { "\u{25b6}" } else { "\u{25bc}" }
                } else {
                    ""
                };
                visible.push((i, format!("{}{}", icon, popup.grep_results[i])));
                i += 1;
                if is_collapsed {
                    while i < popup.grep_results.len() && !popup.grep_is_header[i] {
                        i += 1;
                    }
                }
            }
            let items: Vec<ListItem> = visible
                .iter()
                .map(|(_, t)| ListItem::new(crate::ui::styled_result_line(t, &app.app_theme)))
                .collect();
            (items, "")
        } else if has_title {
            let items: Vec<ListItem> = popup
                .title_results
                .iter()
                .map(|entry| ListItem::new(crate::ui::styled_result_line(entry, &app.app_theme)))
                .collect();
            (items, "")
        } else {
            let msg = if query_text.trim().is_empty() && !has_filter {
                "Type to search notes"
            } else {
                "No results"
            };
            (
                vec![ListItem::new(Span::styled(
                    msg.to_string(),
                    Style::default().fg(app.app_theme.muted),
                ))],
                "",
            )
        };

        let results_list = List::new(all_items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(results_border)
                    .title(results_title),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");
        let mut list_state = ListState::default();
        if results_focused && has_grep {
            let mut vis_pos = 0;
            let mut i = 0;
            while i < popup.grep_results.len() && i <= popup.grep_selected {
                let is_collapsed = popup.grep_is_header[i] && !popup.grep_expanded.contains(&i);
                if i == popup.grep_selected {
                    list_state.select(Some(vis_pos));
                    break;
                }
                vis_pos += 1;
                i += 1;
                if is_collapsed {
                    while i < popup.grep_results.len() && !popup.grep_is_header[i] {
                        i += 1;
                    }
                }
            }
        } else if results_focused && has_title {
            list_state.select(Some(popup.title_selected));
        }
        frame.render_stateful_widget(results_list, results_chunk, &mut list_state);
    }

    if let Some(trash) = &app.popups.trash_view {
        let content = draw_popup_frame(
            frame,
            area,
            "TRASH",
            PopupSize::Large,
            "r restore · d delete · E empty · q close",
            &app.app_theme,
        );

        let border_color = if trash.items.is_empty() {
            app.app_theme.muted
        } else {
            app.app_theme.heading
        };

        let items: Vec<ListItem> = trash
            .items
            .iter()
            .map(|item| {
                let name = item.name.to_string_lossy();
                let when = format_relative_time(item.time_deleted as u64);
                ListItem::new(Line::from(vec![
                    Span::raw(name.to_string()),
                    Span::styled(
                        format!("  ({when})"),
                        Style::default().fg(app.app_theme.muted),
                    ),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color))
                    .title(""),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");

        let mut state = ListState::default();
        state.select(Some(trash.selected));

        frame.render_stateful_widget(list, content, &mut state);
    }

    if let Some(popup) = &app.popups.confirm {
        draw_confirm_popup(frame, popup, area, &app.app_theme);
    }
}

pub(crate) fn list_view_layout(
    area: Rect,
    preview_enabled: bool,
    preview_position: crate::config::PreviewPosition,
    calendar_enabled: bool,
    preview_fullscreen: bool,
) -> (Rect, Option<Rect>, Option<Rect>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    if preview_fullscreen {
        return (chunks[1], Some(chunks[1]), None);
    }

    let (list_column, preview_area) = if preview_enabled {
        let (constraints, list_idx, p_idx) = match preview_position {
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
        let full_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(chunks[1]);
        (full_cols[list_idx], Some(full_cols[p_idx]))
    } else {
        (
            Rect::new(area.x, area.y + 1, area.width, chunks[1].height),
            None,
        )
    };

    let (list_area, calendar_area) = if calendar_enabled {
        let sp = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(9)])
            .split(list_column);
        (sp[0], Some(sp[1]))
    } else {
        (list_column, None)
    };

    (list_area, preview_area, calendar_area)
}

fn get_item_name(app: &App, idx: usize) -> Option<String> {
    if let Some(item) = app.list.visual_list.get(idx) {
        match item {
            crate::list_view::VisualItem::Folder { name, .. } => Some(name.clone()),
            crate::list_view::VisualItem::Note { summary_idx, .. } => {
                app.notes.get(*summary_idx).map(|n| n.title.clone())
            }
            crate::list_view::VisualItem::CreateNew { .. } => Some("Create...".to_string()),
        }
    } else {
        None
    }
}

pub fn get_preview_info(app: &App) -> Option<PreviewHeaderInfo> {
    if !app.preview_fullscreen {
        return None;
    }

    let current_index = if app.mode == ViewMode::Edit {
        if let Some(id) = &app.editor.editing_id {
            if let Some(note_pos) = app.notes.iter().position(|n| &n.id == id) {
                app.list.visual_list.iter().position(|item| {
                    if let crate::list_view::VisualItem::Note { summary_idx, .. } = item {
                        *summary_idx == note_pos
                    } else {
                        false
                    }
                })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        Some(app.list.visual_index)
    };

    if app.mode == ViewMode::Edit {
        if let Some(id) = &app.editor.editing_id {
            if let Some(note) = app.notes.iter().find(|n| &n.id == id) {
                let folder = if note.folder.is_empty() {
                    "Vault".to_string()
                } else {
                    format!("Vault/{}", note.folder)
                };
                let title = crate::events::get_title_text(&app.editor.title_editor).into_owned();
                let title = if title.is_empty() {
                    "Untitled note".to_string()
                } else {
                    title
                };
                let (prev_name, next_name) = if let Some(idx) = current_index {
                    let prev = if idx > 0 {
                        get_item_name(app, idx - 1)
                    } else {
                        None
                    };
                    let next = if idx + 1 < app.list.visual_list.len() {
                        get_item_name(app, idx + 1)
                    } else {
                        None
                    };
                    (prev, next)
                } else {
                    (None, None)
                };
                return Some(PreviewHeaderInfo {
                    path: folder,
                    item_name: title,
                    prev_name,
                    next_name,
                });
            }
        } else if let Some(path) = &app.editor.template_edit_path {
            let parent = path
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let filename = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            let folder = if parent.is_empty() {
                "Templates".to_string()
            } else {
                format!("Templates/{}", parent)
            };
            return Some(PreviewHeaderInfo {
                path: folder,
                item_name: filename,
                prev_name: None,
                next_name: None,
            });
        }
    }

    if let Some(item) = app.list.visual_list.get(app.list.visual_index) {
        let (path, item_name) = match item {
            crate::list_view::VisualItem::Folder { path, name, .. } => {
                if path.is_empty() {
                    ("Vault".to_string(), "Vault".to_string())
                } else if path == VIRTUAL_PINNED_PATH {
                    ("Pinned".to_string(), "Pinned".to_string())
                } else {
                    if let Some(slash_idx) = path.rfind('/') {
                        let parent = &path[..slash_idx];
                        (format!("Vault/{}", parent), name.clone())
                    } else {
                        ("Vault".to_string(), name.clone())
                    }
                }
            }
            crate::list_view::VisualItem::Note { summary_idx, .. } => {
                if let Some(note) = app.notes.get(*summary_idx) {
                    let folder = if note.folder.is_empty() {
                        "Vault".to_string()
                    } else {
                        format!("Vault/{}", note.folder)
                    };
                    (folder, note.title.clone())
                } else {
                    return None;
                }
            }
            crate::list_view::VisualItem::CreateNew { path, .. } => {
                let folder = if path.is_empty() {
                    "Vault".to_string()
                } else {
                    format!("Vault/{}", path)
                };
                (folder, "Create...".to_string())
            }
        };

        let idx = app.list.visual_index;
        let prev_name = if idx > 0 {
            get_item_name(app, idx - 1)
        } else {
            None
        };
        let next_name = if idx + 1 < app.list.visual_list.len() {
            get_item_name(app, idx + 1)
        } else {
            None
        };

        Some(PreviewHeaderInfo {
            path,
            item_name,
            prev_name,
            next_name,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ViewMode;
    use crate::config::PreviewPosition;

    #[test]
    fn calendar_never_overlaps_preview_and_stays_in_list_column() {
        let area = Rect::new(0, 0, 80, 24);
        for &position in &[PreviewPosition::Left, PreviewPosition::Right] {
            let (list_area, preview_area, calendar_area) = list_view_layout(
                area, true, // preview enabled
                position, true,  // calendar enabled
                false, // preview_fullscreen
            );

            let cal = calendar_area.expect("calendar enabled");
            let preview = preview_area.expect("preview enabled");

            // Calendar is strictly underneath list_area.
            assert_eq!(cal.x, list_area.x);
            assert_eq!(cal.width, list_area.width);
            assert_eq!(cal.y, list_area.y + list_area.height);

            // Calendar and preview are disjoint (separated in x since y overlaps).
            let disjoint = cal.right() <= preview.x || preview.right() <= cal.x;
            assert!(disjoint, "calendar must not overlap preview @ {position:?}");
        }
    }

    /// With preview off the calendar spans the full content width at the bottom.
    #[test]
    fn calendar_full_width_when_no_preview() {
        let area = Rect::new(0, 0, 80, 24);
        let (list_area, preview_area, calendar_area) =
            list_view_layout(area, false, PreviewPosition::Right, true, false);

        assert!(preview_area.is_none());
        let cal = calendar_area.expect("calendar enabled");
        assert_eq!(cal.width, list_area.width);
        assert_eq!(cal.y, list_area.y + list_area.height);
        assert_eq!(cal.height, 9);
    }

    /// With the calendar disabled there is no calendar area and list is full.
    #[test]
    fn no_calendar_area_when_disabled() {
        let area = Rect::new(0, 0, 80, 24);
        let (_, preview_area, calendar_area) =
            list_view_layout(area, true, PreviewPosition::Right, false, false);
        assert!(calendar_area.is_none());
        assert!(preview_area.is_some());
    }

    #[test]
    fn test_get_preview_info() {
        let temp_dir = tempfile::tempdir().unwrap();
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&templates_dir).unwrap();

        let storage = crate::storage::Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
        };
        let mut app = App::new(storage).unwrap();

        // 1. preview_fullscreen is false
        app.preview_fullscreen = false;
        assert!(get_preview_info(&app).is_none());

        app.preview_fullscreen = true;

        // 2. ViewMode::List, empty list
        app.mode = ViewMode::List;
        app.list.visual_list.clear();
        assert!(get_preview_info(&app).is_none());

        // 3. ViewMode::List, selected folder
        app.list.visual_list = vec![crate::list_view::VisualItem::Folder {
            path: "work/projects".to_string(),
            name: "projects".to_string(),
            depth: 1,
            is_expanded: false,
            note_count: 0,
        }];
        app.list.visual_index = 0;
        assert_eq!(
            get_preview_info(&app),
            Some(PreviewHeaderInfo {
                path: "Vault/work".to_string(),
                item_name: "projects".to_string(),
                prev_name: None,
                next_name: None,
            })
        );

        // 4. ViewMode::List, selected note with prev and next
        let note = crate::storage::NoteSummary {
            id: "note1".to_string(),
            title: "My Note".to_string(),
            updated_at: 0,
            folder: "work/projects".to_string(),
            tags: vec![],
            pinned: false,
            links: vec![],
            size_bytes: 0,
        };
        app.notes = vec![note];
        app.list.visual_list = vec![
            crate::list_view::VisualItem::Folder {
                path: "work/projects".to_string(),
                name: "projects".to_string(),
                depth: 1,
                is_expanded: true,
                note_count: 1,
            },
            crate::list_view::VisualItem::Note {
                summary_idx: 0,
                depth: 2,
                is_clin: false,
                is_draw: false,
                is_canvas: false,
                in_virtual_pinned_folder: false,
            },
            crate::list_view::VisualItem::Folder {
                path: "other".to_string(),
                name: "other".to_string(),
                depth: 1,
                is_expanded: false,
                note_count: 0,
            },
        ];
        app.list.visual_index = 1;
        assert_eq!(
            get_preview_info(&app),
            Some(PreviewHeaderInfo {
                path: "Vault/work/projects".to_string(),
                item_name: "My Note".to_string(),
                prev_name: Some("projects".to_string()),
                next_name: Some("other".to_string()),
            })
        );

        // 5. ViewMode::Edit, editing a note
        app.mode = ViewMode::Edit;
        app.editor.editing_id = Some("note1".to_string());
        app.editor.title_editor = crate::events::make_title_editor(
            "Unsaved Title Change",
            app.app_theme.highlight_fg,
            app.app_theme.highlight_bg,
        );
        assert_eq!(
            get_preview_info(&app),
            Some(PreviewHeaderInfo {
                path: "Vault/work/projects".to_string(),
                item_name: "Unsaved Title Change".to_string(),
                prev_name: Some("projects".to_string()),
                next_name: Some("other".to_string()),
            })
        );
    }
}
