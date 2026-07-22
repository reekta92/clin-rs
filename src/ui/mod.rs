use anyhow::{Context, Result};
use ratatui::{prelude::*, widgets::*};
use ratatui_textarea::TextArea;
use std::path::Path;
use std::process::Command;

use crate::app::{App, EditFocus, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::overlay::OverlayView;

pub(crate) mod braille;
mod edit_view;
mod help;
pub(crate) mod help_content;
mod list_view;
mod popups;
pub(crate) mod quick_search;
pub(crate) mod scrollbar;
pub(crate) mod setup;
mod title_bar;

pub use edit_view::draw_edit_view;
pub(crate) use edit_view::render_editor_widget;
pub use help::*;
pub use help_content::{HelpSuggestion, roll_suggestions};
pub(crate) use list_view::{
    draw_list_view, get_preview_info,
    list_detail_line, list_view_layout, section_rects,
};
pub use popups::*;
pub use setup::draw_setup_view;
pub use title_bar::*;

use crate::config::IconMode;

/// Return the appropriate icon string based on `mode`.
pub fn get_icon(nerd: &'static str, unicode: &'static str, mode: IconMode) -> &'static str {
    match mode {
        IconMode::Nerd => nerd,
        IconMode::Unicode => unicode,
        IconMode::None => "",
    }
}

/// Return the appropriate icon character based on `mode`.
pub fn get_char(nerd: char, unicode: char, mode: IconMode) -> char {
    match mode {
        IconMode::Nerd => nerd,
        IconMode::Unicode => unicode,
        IconMode::None => ' ',
    }
}

pub fn tab_vec_from_array<'a>(arr: &[(&'a str, &'a str)]) -> Vec<(&'a str, Option<&'a str>)> {
    arr.iter().map(|&(l, g)| (l, Some(g))).collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PopupSize {
    Small,   // 40% width, 40% height. Max bounds: 60 cols x 20 rows
    Medium,  // 50% width, 50% height. Max bounds: 80 cols x 30 rows
    Large,   // 60% width, 60% height. Max bounds: 100 cols x 40 rows
    Prompt,  // 50% width. Fixed 5 height. Max bounds: 80 cols wide
    Confirm, // 50% width. Fixed 12 height. Max bounds: 80 cols wide
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewHeaderInfo {
    pub path: String,
    pub item_name: String,
    pub prev_name: Option<String>,
    pub next_name: Option<String>,
}

pub fn draw_ui(frame: &mut Frame, app: &mut App, focus: EditFocus) {
    // Suppress background element hover when a popup is active
    let popup_hover_pos = app.mouse_pos;
    if app.popups.active.is_some() || app.command_palette.is_some() || app.popups.confirm.is_some()
    {
        app.mouse_pos = None;
    }

    if let Some(_bg) = app.app_theme.bg {
        let block = Block::default().style(app.app_theme.bg_style());
        frame.render_widget(block, frame.area());
    }

    match app.mode {
        ViewMode::List => draw_list_view(frame, app),
        ViewMode::Edit => draw_edit_view(frame, app, focus),
        ViewMode::Help => draw_help_view(frame, app),
        ViewMode::Setup => draw_setup_view(frame, app),
        ViewMode::Graph => {
            if let Some(mut graf) = app.graph_state.take() {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());

                {
                    let guard;
                    let mut ctx = crate::statusline::StatuslineContext::for_overlay(
                        &app.config,
                        ViewMode::Graph,
                    );
                    ctx.area = Some(outer[0]);
                    ctx.app = Some(app);
                    ctx.app_status = Some(app.status.as_ref());
                    ctx.vault_path = Some(&app.storage.data_dir);
                    ctx.date_format = Some(&app.date_format);
                    ctx.graph_fps = graf.canvas_fps();
                    if let Some(graph_state) = &graf.graph_state {
                        guard = graph_state.read();
                        ctx.graph = Some(&guard);
                    }
                    let (left_line, right_line) = crate::statusline::render_header(
                        &ctx,
                        &app.config.statusline,
                        ViewMode::Graph,
                        &app.app_theme,
                    );
                    draw_view_title_bar(
                        frame,
                        outer[0],
                        &app.app_theme,
                        left_line,
                        right_line,
                        Some(app.status.as_ref()),
                        app.load_spinner_tick,
                    );
                }

                graf.overlay_render(frame, outer[1], app);
                app.graph_state = Some(graf);
            }
        }

        ViewMode::Draw => {
            if let Some(mut draw) = app.draw_state.take() {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                let icon_mode = app.config.ui.icon_mode;
                let tabs_arr = crate::draw::render::draw_tool_tabs(icon_mode);
                let tabs = tab_vec_from_array(&tabs_arr);
                let active = crate::draw::render::draw_tool_tab_index(draw.active_tool);
                let hovered = app.mouse_pos.and_then(|(col, row)| {
                    if row == outer[0].y {
                        let region = crate::ui::title_bar_tabs_region(outer[0], "Draw");
                        crate::ui::hit_test_tabs(
                            &tabs,
                            outer[0].x,
                            outer[0].width,
                            region.x,
                            col,
                            false,
                            icon_mode,
                        )
                    } else {
                        None
                    }
                });
                let spans =
                    build_tab_spans(&tabs, active, hovered, &app.app_theme, false, icon_mode);
                let mut ctx =
                    crate::statusline::StatuslineContext::for_overlay(&app.config, ViewMode::Draw);
                ctx.area = Some(outer[0]);
                ctx.app_status = Some(app.status.as_ref());
                ctx.vault_path = Some(&app.storage.data_dir);
                ctx.date_format = Some(&app.date_format);
                ctx.draw = Some(&draw);
                let (left_line, right_line) = crate::statusline::render_header(
                    &ctx,
                    &app.config.statusline,
                    ViewMode::Draw,
                    &app.app_theme,
                );
                draw_view_title_bar_with_tabs(
                    frame,
                    outer[0],
                    "Draw",
                    &app.app_theme,
                    left_line,
                    spans,
                    right_line,
                    Some(app.status.as_ref()),
                    app.load_spinner_tick,
                );
                draw.mouse_pos = app.mouse_pos;
                draw.overlay_render(frame, outer[1], app);
                app.draw_state = Some(draw);
            }
        }
        ViewMode::Canvas => {
            if let Some(mut canvas) = app.canvas_state.take() {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                let mut ctx = crate::statusline::StatuslineContext::for_overlay(
                    &app.config,
                    ViewMode::Canvas,
                );
                ctx.area = Some(outer[0]);
                ctx.app_status = Some(app.status.as_ref());
                ctx.vault_path = Some(&app.storage.data_dir);
                ctx.date_format = Some(&app.date_format);
                ctx.canvas = Some(&canvas);
                let (left_line, right_line) = crate::statusline::render_header(
                    &ctx,
                    &app.config.statusline,
                    ViewMode::Canvas,
                    &app.app_theme,
                );
                draw_view_title_bar(
                    frame,
                    outer[0],
                    &app.app_theme,
                    left_line,
                    right_line,
                    Some(app.status.as_ref()),
                    app.load_spinner_tick,
                );
                canvas.overlay_render(frame, outer[1], app);
                app.canvas_state = Some(canvas);
            }
        }
        ViewMode::Backup => {
            if let Some(mut backup) = app.backup_state.take() {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                // Backup uses a custom header render instead of the standard title bar
                crate::backup::render::draw_header(
                    frame,
                    outer[0],
                    &app.config,
                    Some(app.status.as_ref()),
                    &app.storage.data_dir,
                    &app.date_format,
                    &backup,
                    app.config.ui.icon_mode,
                    app.mouse_pos,
                );
                backup.mouse_pos = app.mouse_pos;
                backup.overlay_render(frame, outer[1], app);
                app.backup_state = Some(backup);
            }
        }
        ViewMode::Outline => {
            if let Some(mut tree) = app.outline_state.take() {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                let mut ctx = crate::statusline::StatuslineContext::for_overlay(
                    &app.config,
                    ViewMode::Outline,
                );
                ctx.area = Some(outer[0]);
                ctx.app_status = Some(app.status.as_ref());
                ctx.vault_path = Some(&app.storage.data_dir);
                ctx.date_format = Some(&app.date_format);
                ctx.outline = Some(&tree);
                let (left_line, right_line) = crate::statusline::render_header(
                    &ctx,
                    &app.config.statusline,
                    ViewMode::Outline,
                    &app.app_theme,
                );
                draw_view_title_bar(
                    frame,
                    outer[0],
                    &app.app_theme,
                    left_line,
                    right_line,
                    Some(app.status.as_ref()),
                    app.load_spinner_tick,
                );
                tree.mouse_pos = app.mouse_pos;
                tree.overlay_render(frame, outer[1], app);
                app.outline_state = Some(tree);
            }
        }
    }
    // Restore mouse_pos for popup hover detection
    app.mouse_pos = popup_hover_pos;

    // Global popups — rendered on top of the active view
    // Template popup
    if let Some(crate::popups::ActivePopup::Template(popup)) = &mut app.popups.active {
        draw_template_popup(frame, popup, frame.area(), &app.app_theme, app.mouse_pos);
    }

    // Folder popup
    if let Some(crate::popups::ActivePopup::Folder(popup)) = &mut app.popups.active {
        let title = match popup.mode {
            crate::popups::FolderPopupMode::Create { .. } => "NEW FOLDER",
            crate::popups::FolderPopupMode::Rename { .. } => "RENAME FOLDER",
        };
        let content = draw_popup_frame(
            frame,
            frame.area(),
            title,
            PopupSize::Prompt,
            PopupHints::Keybinds(&[
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::Confirm),
                    "confirm",
                ),
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::Cancel),
                    "cancel",
                ),
            ]),
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

    // Tag popup
    if let Some(crate::popups::ActivePopup::Tag(popup)) = &mut app.popups.active {
        let suggestion_height = if popup.suggestions.is_empty() {
            0u16
        } else {
            (popup.suggestions.len() as u16).clamp(1, 5)
        };
        let content = draw_popup_frame(
            frame,
            frame.area(),
            "TAGS",
            PopupSize::Large,
            PopupHints::Keybinds(&[
                ("Ctrl+S".to_string(), "batch assign"),
                ("Tab".to_string(), "accept"),
                ("Enter".to_string(), "save"),
                ("d".to_string(), "delete from all"),
                ("Esc".to_string(), "cancel"),
            ]),
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
            crate::ui::empty_list_item(&app.app_theme, "No tags found")
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

        let state = crate::ui::render_list_with_selection(
            frame,
            tags_list,
            chunks[1],
            (popup.focus == crate::popups::TagPopupFocus::AllTagsList
                && !popup.all_tags.is_empty())
            .then_some(popup.all_tags_selected),
            popup.scroll_offset,
        );
        popup.scroll_offset = state.offset();
        let inner_tags = Rect {
            x: chunks[1].x + 1,
            y: chunks[1].y + 1,
            width: chunks[1].width.saturating_sub(2),
            height: chunks[1].height.saturating_sub(2),
        };
        crate::ui::paint_list_hover(
            frame,
            inner_tags,
            &state,
            popup.all_tags.len(),
            app.mouse_pos,
            app.app_theme.hover_style(),
        );
        popup.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track: crate::ui::scrollbar::track_rect(inner_tags),
            content_len: popup.all_tags.len(),
            viewport_len: inner_tags.height as usize,
        });
        crate::ui::scrollbar::draw_scrollbar(
            frame,
            inner_tags,
            popup.all_tags.len(),
            inner_tags.height as usize,
            popup.all_tags_selected,
            popup.all_tags.len().saturating_sub(1),
            &app.app_theme,
        );
    }

    // Folder picker popup
    if let Some(crate::popups::ActivePopup::FolderPicker(picker)) = &mut app.popups.active {
        let title = match picker.mode {
            crate::popups::FolderPickerMode::CopyNote { .. }
            | crate::popups::FolderPickerMode::BulkCopyNotes { .. }
            | crate::popups::FolderPickerMode::BulkCopyFolders { .. }
            | crate::popups::FolderPickerMode::BulkCopyMixed { .. } => "COPY",
            _ => "MOVE",
        };
        let content = draw_popup_frame(
            frame,
            frame.area(),
            title,
            PopupSize::Large,
            PopupHints::Keybinds(&[
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::CycleFocus),
                    "switch",
                ),
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::Confirm),
                    "confirm",
                ),
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::Cancel),
                    "cancel",
                ),
            ]),
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
            crate::ui::empty_list_item(&app.app_theme, "(no matching folders)")
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

        let state = crate::ui::render_list_with_selection(
            frame,
            list,
            chunks[1],
            (picker.focus == crate::app::FolderPickerFocus::Results
                && !picker.filtered_folders.is_empty())
            .then_some(picker.selected),
            picker.scroll_offset,
        );
        picker.scroll_offset = state.offset();
        let inner = Rect {
            x: chunks[1].x + 1,
            y: chunks[1].y + 1,
            width: chunks[1].width.saturating_sub(2),
            height: chunks[1].height.saturating_sub(2),
        };
        crate::ui::paint_list_hover(
            frame,
            inner,
            &state,
            picker.filtered_folders.len(),
            app.mouse_pos,
            app.app_theme.hover_style(),
        );
        picker.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track: crate::ui::scrollbar::track_rect(inner),
            content_len: picker.filtered_folders.len(),
            viewport_len: inner.height as usize,
        });
        crate::ui::scrollbar::draw_scrollbar(
            frame,
            inner,
            picker.filtered_folders.len(),
            inner.height as usize,
            picker.selected,
            picker.filtered_folders.len().saturating_sub(1),
            &app.app_theme,
        );
    }

    // Command palette
    if let Some(palette) = &mut app.command_palette {
        let area = frame.area();
        let content = draw_popup_frame(
            frame,
            area,
            "COMMANDS",
            PopupSize::Large,
            PopupHints::Keybinds(&[
                ("Tab".to_string(), "category"),
                ("Enter".to_string(), "run"),
                ("↑/↓".to_string(), "select"),
                ("Esc".to_string(), "close"),
            ]),
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

        let tabs: Vec<(&str, Option<&str>)> = crate::palette::palette_tabs(app.config.ui.icon_mode)
            .iter()
            .map(|(l, g, _)| (*l, Some(*g)))
            .collect();
        let hovered = app.mouse_pos.and_then(|(col, row)| {
            if row == chunks[1].y {
                crate::ui::hit_test_tabs(
                    &tabs,
                    chunks[1].x,
                    chunks[1].width,
                    chunks[1].x,
                    col,
                    app.config.ui.tab_icons_only,
                    app.config.ui.icon_mode,
                )
            } else {
                None
            }
        });
        let tab_spans = build_tab_spans(
            &tabs,
            palette.active_tab,
            hovered,
            &app.app_theme,
            app.config.ui.tab_icons_only,
            app.config.ui.icon_mode,
        );
        let tabs_w = Paragraph::new(Line::from(tab_spans))
            .alignment(Alignment::Center)
            .style(app.app_theme.hint_line_bg_style());
        frame.render_widget(tabs_w, chunks[1]);

        let hovered_cmd_idx = app.mouse_pos.and_then(|(col, row)| {
            let inner_y = chunks[2].y + 1;
            if !palette.items.is_empty()
                && row >= inner_y
                && col > chunks[2].x
                && col < chunks[2].x + chunks[2].width - 1
            {
                crate::ui::list_index_at(
                    row,
                    inner_y,
                    2,
                    palette.state.offset(),
                    palette.items.len(),
                )
            } else {
                None
            }
        });
        let items: Vec<ListItem> = palette
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let mut spans = vec![Span::styled(
                    format!("{} ", item.glyph),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                )];
                spans.extend(crate::ui::style_palette_name(&item.name, &app.app_theme));
                let mut list_item = ListItem::new(vec![
                    Line::from(spans),
                    Line::from(Span::styled(
                        &item.description,
                        Style::default().fg(app.app_theme.muted),
                    )),
                ]);
                if Some(i) == hovered_cmd_idx {
                    list_item = list_item.style(app.app_theme.hover_style());
                }
                list_item
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
        let content_len = palette.items.len();
        if content_len > 0 {
            let viewport_len = ((chunks[2].height.saturating_sub(2)) / 2) as usize;
            let meta = crate::ui::scrollbar::ScrollbarMeta {
                track: crate::ui::scrollbar::track_rect(chunks[2]),
                content_len,
                viewport_len,
            };
            palette.last_scroll = Some(meta);
            palette.last_results_area = Some(chunks[2]);
            if app.config.ui.scrollbars {
                let pos = palette.state.selected().unwrap_or(0);
                crate::ui::scrollbar::draw_scrollbar(
                    frame,
                    chunks[2],
                    content_len,
                    viewport_len,
                    pos,
                    content_len.saturating_sub(1),
                    &app.app_theme,
                );
            }
        }
    }

    // Note rename popup
    if let Some(crate::popups::ActivePopup::NoteRename(popup)) = &mut app.popups.active {
        let content = draw_popup_frame(
            frame,
            frame.area(),
            "RENAME",
            PopupSize::Prompt,
            PopupHints::Keybinds(&[
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::Confirm),
                    "rename",
                ),
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::Cancel),
                    "cancel",
                ),
            ]),
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

    // Goals popup
    if let Some(crate::popups::ActivePopup::Goals(popup)) = &mut app.popups.active {
        let (title, keybinds) = match popup.mode {
            crate::popups::GoalsPopupMode::WordGoal => (
                "DAILY WORD GOAL",
                vec![
                    ("Enter".to_string(), "word count"),
                    ("Esc".to_string(), "cancel"),
                ],
            ),
            crate::popups::GoalsPopupMode::NoteGoal => (
                "DAILY NOTE GOAL",
                vec![
                    ("Enter".to_string(), "note count"),
                    ("Esc".to_string(), "cancel"),
                ],
            ),
        };
        let content = draw_popup_frame(
            frame,
            frame.area(),
            title,
            PopupSize::Prompt,
            PopupHints::Keybinds(&keybinds),
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

    // Create note popup
    if let Some(crate::popups::ActivePopup::CreateNote(popup, format)) = &mut app.popups.active {
        let title = match format {
            crate::popups::NoteFormat::Markdown => "NEW NOTE",
            crate::popups::NoteFormat::Draw => "NEW DRAWING",
            crate::popups::NoteFormat::Canvas => "NEW CANVAS",
            crate::popups::NoteFormat::PlainText => "NEW TEXT FILE",
        };
        let content = draw_popup_frame(
            frame,
            frame.area(),
            title,
            PopupSize::Prompt,
            PopupHints::Keybinds(&[
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::Confirm),
                    "create",
                ),
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::Cancel),
                    "cancel",
                ),
            ]),
            &app.app_theme,
        );
        popup.input.set_block(popup_block("", &app.app_theme));
        frame.render_widget(&popup.input, content);
    }

    // Import popup
    if let Some(crate::popups::ActivePopup::Import(popup)) = &mut app.popups.active {
        let title = match popup.source {
            crate::popups::ImportSource::File => "IMPORT FILE",
            crate::popups::ImportSource::Csv => "IMPORT CSV/TSV",
            crate::popups::ImportSource::Json => "IMPORT JSON",
            crate::popups::ImportSource::Url => "IMPORT URL",
            crate::popups::ImportSource::Clipboard => "IMPORT CLIPBOARD",
        };
        let content = draw_popup_frame(
            frame,
            frame.area(),
            title,
            PopupSize::Large,
            PopupHints::Keybinds(&[
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::Confirm),
                    "import",
                ),
                (
                    app.keybinds
                        .display_list(crate::keybinds::ListAction::Cancel),
                    "cancel",
                ),
            ]),
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

    // Search popup
    if let Some(crate::popups::ActivePopup::Search(popup)) = &mut app.popups.active {
        let area = frame.area();
        let content = draw_popup_frame(
            frame,
            area,
            "SEARCH",
            PopupSize::Large,
            PopupHints::Keybinds(&[
                ("f:".to_string(), "folder"),
                ("p:".to_string(), "pinned"),
                ("t:".to_string(), "tag"),
                ("g:".to_string(), "text"),
                ("\\e\\".to_string(), "escapes filters"),
            ]),
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
            let add_sep =
                |spans: &mut Vec<Span<'static>>, first: &mut bool, theme: &AppThemeColors| {
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
                    crate::ui::get_icon("\u{f07c}", "\u{1f4c2}", app.config.ui.icon_mode)
                        .to_string(),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if parsed.pinned_only {
                add_sep(&mut spans, &mut first, &app.app_theme);
                let pin_icon =
                    crate::ui::get_icon("\u{f08d}", "\u{1f4cc}", app.config.ui.icon_mode);
                spans.push(Span::styled(
                    format!("{pin_icon} Pinned"),
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
                let search_icon =
                    crate::ui::get_icon("\u{f002}", "\u{1f50d}", app.config.ui.icon_mode);
                spans.push(Span::styled(
                    format!("{search_icon} {grep_display}"),
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
                    crate::ui::get_icon("\u{f02b}", "\u{1f3f7}", app.config.ui.icon_mode)
                        .to_string(),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::raw(" "));
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

        let has_title = !popup.title_result_ids.is_empty();
        let has_grep = !popup.grep_results.is_empty();

        let results_focused = popup.focus == crate::popups::SearchFocus::Results;
        let results_border = if results_focused {
            Style::default().fg(app.app_theme.heading)
        } else {
            Style::default().fg(app.app_theme.muted)
        };

        let inner_results = Rect {
            x: results_chunk.x + 1,
            y: results_chunk.y + 1,
            width: results_chunk.width.saturating_sub(2),
            height: results_chunk.height.saturating_sub(2),
        };
        let viewport_len = inner_results.height as usize;

        let total_items = if has_grep {
            popup.total_grep_rows()
        } else if has_title {
            popup.title_result_ids.len()
        } else {
            0
        };

        let selected_idx = if has_grep {
            popup.grep_selected
        } else if has_title {
            popup.title_selected
        } else {
            0
        };

        let mut offset = popup.results_scroll_offset;
        if offset > total_items.saturating_sub(viewport_len) {
            offset = total_items.saturating_sub(viewport_len);
        }
        if selected_idx < offset {
            offset = selected_idx;
        } else if selected_idx >= offset + viewport_len {
            offset = selected_idx.saturating_add(1).saturating_sub(viewport_len);
        }
        popup.results_scroll_offset = offset;

        let end = (offset + viewport_len).min(total_items);

        let items: Vec<ListItem> = if has_grep {
            (offset..end)
                .map(|r| {
                    if popup.globally_truncated && r == popup.total_grep_rows() - 1 {
                        ListItem::new(Span::styled(
                            "  Results truncated; refine grep query",
                            Style::default()
                                .fg(app.app_theme.muted)
                                .add_modifier(Modifier::ITALIC),
                        ))
                    } else {
                        let hit_idx = match popup.grep_row_offsets.binary_search(&r) {
                            Ok(i) => i,
                            Err(i) => i.saturating_sub(1),
                        };
                        let base = popup.grep_row_offsets[hit_idx];
                        let hit = &popup.grep_results[hit_idx];
                        if r == base {
                            let arrow = if popup.grep_expanded.contains(&hit.note_id) {
                                "▼ "
                            } else {
                                "▶ "
                            };
                            let note_summary =
                                app.notes.iter().find(|n| n.id.as_str() == &*hit.note_id);
                            let title = note_summary
                                .map(|n| n.title.as_str())
                                .unwrap_or(&*hit.note_id);
                            let folder = note_summary.map(|n| n.folder.as_str()).unwrap_or("");
                            let label = if folder.is_empty() {
                                title.to_string()
                            } else {
                                format!("{folder}/{title}")
                            };
                            let trunc_suffix = if hit.truncated {
                                "; first 200 lines shown"
                            } else {
                                ""
                            };
                            let header_text =
                                format!("{arrow}{label} ({}{trunc_suffix})", hit.match_count);
                            ListItem::new(crate::ui::styled_result_line(
                                &header_text,
                                &app.app_theme,
                                app.config.ui.icon_mode,
                            ))
                        } else {
                            let line_idx = r - base - 1;
                            let line_hit = &hit.lines[line_idx];
                            let line_text =
                                format!("  L{}: {}", line_hit.line_number, line_hit.snippet);
                            ListItem::new(Span::styled(
                                line_text,
                                Style::default().fg(app.app_theme.text),
                            ))
                        }
                    }
                })
                .collect()
        } else if has_title {
            (offset..end)
                .map(|idx| {
                    let id_arc = &popup.title_result_ids[idx];
                    let note_summary = app.notes.iter().find(|n| n.id.as_str() == &**id_arc);
                    let title = note_summary.map(|n| n.title.as_str()).unwrap_or(&**id_arc);
                    let folder = note_summary.map(|n| n.folder.as_str()).unwrap_or("");
                    let label = if folder.is_empty() {
                        title.to_string()
                    } else {
                        format!("{folder}/{title}")
                    };
                    ListItem::new(crate::ui::styled_result_line(
                        &label,
                        &app.app_theme,
                        app.config.ui.icon_mode,
                    ))
                })
                .collect()
        } else {
            let msg = if query_text.trim().is_empty() && !has_filter {
                "Type to search notes"
            } else {
                "No results"
            };
            vec![ListItem::new(Span::styled(
                msg,
                Style::default().fg(app.app_theme.muted),
            ))]
        };

        let rel_selected = selected_idx.saturating_sub(offset);
        let mut rel_state = ListState::default();
        if results_focused && total_items > 0 {
            rel_state.select(Some(rel_selected));
        }

        let results_list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(results_border),
            )
            .highlight_style(
                Style::default()
                    .fg(app.app_theme.highlight_fg)
                    .bg(app.app_theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("  ");

        frame.render_stateful_widget(results_list, results_chunk, &mut rel_state);
        crate::ui::paint_list_hover(
            frame,
            inner_results,
            &rel_state,
            end.saturating_sub(offset),
            app.mouse_pos,
            app.app_theme.hover_style(),
        );
        popup.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track: crate::ui::scrollbar::track_rect(inner_results),
            content_len: total_items,
            viewport_len,
        });
        if app.config.ui.scrollbars {
            crate::ui::scrollbar::draw_scrollbar(
                frame,
                inner_results,
                total_items,
                viewport_len,
                selected_idx,
                total_items.saturating_sub(1),
                &app.app_theme,
            );
        }
    }
    // Trash view popup
    if let Some(crate::popups::ActivePopup::TrashView(trash)) = &mut app.popups.active {
        let area = frame.area();
        let content = draw_popup_frame(
            frame,
            area,
            "TRASH",
            PopupSize::Large,
            PopupHints::Keybinds(&[
                ("r".to_string(), "restore"),
                ("d".to_string(), "delete"),
                ("E".to_string(), "empty"),
                ("q".to_string(), "close"),
            ]),
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
                let when = crate::ui::format_relative_time(item.time_deleted as u64);
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

        let state = crate::ui::render_list_with_selection(
            frame,
            list,
            content,
            Some(trash.selected),
            trash.scroll_offset,
        );
        trash.scroll_offset = state.offset();
        let inner = Rect {
            x: content.x + 1,
            y: content.y + 1,
            width: content.width.saturating_sub(2),
            height: content.height.saturating_sub(2),
        };
        crate::ui::paint_list_hover(
            frame,
            inner,
            &state,
            trash.items.len(),
            app.mouse_pos,
            app.app_theme.hover_style(),
        );
        trash.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track: crate::ui::scrollbar::track_rect(inner),
            content_len: trash.items.len(),
            viewport_len: inner.height as usize,
        });
        crate::ui::scrollbar::draw_scrollbar(
            frame,
            inner,
            trash.items.len(),
            inner.height as usize,
            trash.selected,
            trash.items.len().saturating_sub(1),
            &app.app_theme,
        );
    }

    // Confirm popup
    if let Some(popup) = &app.popups.confirm {
        draw_confirm_popup(frame, popup, frame.area(), &app.app_theme);
    }

    if let Some(popup) = &mut app.popups.active {
        popup.draw(
            frame,
            frame.area(),
            &app.app_theme,
            &app.keybinds,
            app.mouse_pos,
        );
    }

    // Context menu (from edit view)
    if let Some(crate::popups::ActivePopup::ContextMenu(menu)) = &app.popups.active {
        let menu_width = menu.items.iter().map(|l| l.len() as u16).max().unwrap_or(0);
        let menu_height = menu.items.len() as u16;
        let x = menu.x.min(frame.area().width.saturating_sub(menu_width));
        let y = menu.y.min(frame.area().height.saturating_sub(menu_height));
        let menu_area = Rect::new(x, y, menu_width, menu_height);
        let items: Vec<ListItem> = menu.items.iter().map(|l| ListItem::new(*l)).collect();
        let list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.preview_bg_style())
                    .borders(Borders::NONE),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_widget(Clear, menu_area);
        let state =
            crate::ui::render_list_with_selection(frame, list, menu_area, Some(menu.selected), 0);
        crate::ui::paint_list_hover(
            frame,
            menu_area,
            &state,
            menu.items.len(),
            app.mouse_pos,
            app.app_theme.hover_style(),
        );
    }
}

pub fn open_in_file_manager(path: &Path) -> Result<()> {
    use std::process::Stdio;

    let command = if cfg!(target_os = "linux") {
        "xdg-open"
    } else if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        anyhow::bail!("opening file manager is not supported on this platform")
    };

    Command::new(command)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to launch {command}"))?;
    Ok(())
}

pub fn open_with_default_application(path: &Path) -> Result<()> {
    use std::process::Stdio;

    let command = if cfg!(target_os = "linux") {
        "xdg-open"
    } else if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "cmd"
    } else {
        anyhow::bail!("opening files is not supported on this platform")
    };

    let mut cmd = Command::new(command);
    if cfg!(target_os = "windows") {
        // `cmd /C start "" <path>` invokes the associated application.
        cmd.arg("/C").arg("start").arg("");
    }
    cmd.arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to launch {command}"))?;
    Ok(())
}

/// Open a file-pick dialog. `filter_ext` supports semicolon-separated
/// extensions (e.g. `"png;jpg"`) which are formatted per-platform.
pub fn pick_file(filter_name: &str, filter_ext: &str) -> Result<Option<String>> {
    // Build per-platform filter string from semicolon-separated extensions
    let ext_list: Vec<&str> = filter_ext.split(';').collect();
    let zenity_glob = ext_list
        .iter()
        .map(|e| format!("*.{}", e.trim_start_matches('.')))
        .collect::<Vec<_>>()
        .join(" ");
    let kdialog_glob = if ext_list.len() == 1 {
        format!("*.{}", ext_list[0].trim_start_matches('.'))
    } else {
        let exts: Vec<&str> = ext_list.iter().map(|e| e.trim_start_matches('.')).collect();
        let mut s = String::from("*.{");
        s.push_str(&exts.join(","));
        s.push('}');
        s
    };

    if cfg!(target_os = "linux") {
        if which::which("zenity").is_ok() {
            let output = Command::new("zenity")
                .arg("--file-selection")
                .arg(format!("--file-filter={filter_name} | {zenity_glob}"))
                .output()?;
            if output.status.success() {
                return Ok(Some(
                    String::from_utf8_lossy(&output.stdout).trim().to_string(),
                ));
            }
        } else if which::which("kdialog").is_ok() {
            let output = Command::new("kdialog")
                .arg("--getopenfilename")
                .arg(".")
                .arg(&kdialog_glob)
                .output()?;
            if output.status.success() {
                return Ok(Some(
                    String::from_utf8_lossy(&output.stdout).trim().to_string(),
                ));
            }
        }
    } else if cfg!(target_os = "macos") {
        // macOS expects UTIs, not extensions. Use first extension as fallback.
        let first = ext_list
            .first()
            .copied()
            .unwrap_or("")
            .trim_start_matches('.');
        let posix_script = format!(
            "POSIX path of (choose file with prompt \"Select a {} file\" of type {{\"{}\"}})",
            filter_name, first,
        );
        let output = Command::new("osascript")
            .arg("-e")
            .arg(posix_script)
            .output()?;
        if output.status.success() {
            return Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ));
        }
    } else if cfg!(target_os = "windows") {
        let win_filter = ext_list
            .iter()
            .map(|e| format!("*.{}", e.trim_start_matches('.')))
            .collect::<Vec<_>>()
            .join(";");
        let ps_script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; \
             $f = New-Object System.Windows.Forms.OpenFileDialog; \
             $f.Filter = '{filter_name} ({win_filter})|{win_filter}'; \
             $f.ShowDialog() | Out-Null; $f.FileName"
        );
        let output = Command::new("powershell")
            .arg("-Command")
            .arg(ps_script)
            .output()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(Some(path));
            }
        }
    }

    Ok(None)
}

/// Replicate ratatui-textarea 0.9.2's viewport auto-scroll math (widget.rs
/// `next_scroll_top` + `scroll_top_col`) so we can track the viewport offset
/// in O(1) without Debug-formatting the entire textarea on every mouse event.
/// `prev_*` are the cached offsets from the previous frame; returns the new ones.
pub fn refresh_textarea_viewport(
    textarea: &TextArea,
    prev_row: u16,
    prev_col: u16,
    area: Rect,
    line_numbers: bool,
) -> (u16, u16) {
    fn next_top(prev_top: u16, cursor: u16, len: u16) -> u16 {
        if cursor < prev_top {
            cursor
        } else if prev_top.saturating_add(len) <= cursor {
            cursor + 1 - len
        } else {
            prev_top
        }
    }
    let inner = textarea.block().map(|b| b.inner(area)).unwrap_or(area);
    if inner.width == 0 || inner.height == 0 {
        return (prev_row, prev_col);
    }
    let sc = textarea.screen_cursor();
    let row = next_top(prev_row, sc.row as u16, inner.height);

    // Column math mirrors scroll_top_col: the line-number gutter shifts the
    // effective cursor column used for horizontal auto-scroll.
    let mut col_cursor = sc.col as u16;
    if line_numbers {
        let lnum = textarea.lines().len().to_string().len() as u16 + 2; // digits + 2 margins
        if col_cursor <= lnum {
            col_cursor = col_cursor.saturating_mul(2);
        } else {
            col_cursor = col_cursor.saturating_add(lnum);
        }
    }
    let col = next_top(prev_col, col_cursor, inner.width);
    (row, col)
}

/// Fallback: parse viewport from Debug output. Used by popup/pinstar textareas that
/// don't have cached viewport offsets. Do not use in the hot mouse-drag path.
pub fn get_textarea_scroll(textarea: &TextArea) -> (usize, usize) {
    let mut scroll_row = 0;
    let mut scroll_col = 0;

    let debug_str = format!("{textarea:?}");
    if let Some(start) = debug_str.find("viewport: Viewport(") {
        let after_start = &debug_str[start + "viewport: Viewport(".len()..];
        let number_str = if let Some(inner) = after_start.strip_prefix("AtomicU64(") {
            if let Some(end) = inner.find(')') {
                &inner[..end]
            } else {
                ""
            }
        } else if let Some(end) = after_start.find(')') {
            &after_start[..end]
        } else {
            ""
        };
        if let Ok(number) = number_str.parse::<u64>() {
            scroll_row = ((number >> 16) & 0xFFFF) as usize;
            scroll_col = (number & 0xFFFF) as usize;
        }
    }
    (scroll_row, scroll_col)
}
pub fn line_number_gutter(
    line_count: usize,
    cursor_row: usize,
    scroll_row: usize,
    height: u16,
    theme: &AppThemeColors,
    top_padding: u16,
) -> Paragraph<'static> {
    let digits = line_count.max(1).to_string().len();
    let display_lines = height as usize;
    let mut gutter_lines: Vec<Line<'static>> = Vec::with_capacity(display_lines);
    for i in 0..display_lines.min(line_count.saturating_sub(scroll_row)) {
        let current_line_idx = i + scroll_row;
        let is_current = current_line_idx == cursor_row;
        let style = if is_current {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        };
        gutter_lines.push(Line::from(vec![Span::styled(
            format!("{:>width$} ", current_line_idx + 1, width = digits),
            style,
        )]));
    }
    for _ in gutter_lines.len()..display_lines {
        gutter_lines.push(Line::from(Span::raw(" ")));
    }
    Paragraph::new(gutter_lines)
        .style(theme.preview_bg_style())
        .block(
            Block::default()
                .padding(Padding::new(0, 0, top_padding, 0))
                .style(theme.preview_bg_style()),
        )
}

pub fn render_textarea_with_theme(
    frame: &mut Frame,
    textarea: &mut TextArea,
    area: Rect,
    theme: &AppThemeColors,
    has_focus: bool,
    show_line_numbers: bool,
    block: Block<'static>,
    base_style: Style,
) {
    textarea.set_block(block);
    textarea.set_style(base_style);
    textarea.set_cursor_style(if has_focus {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    });
    textarea.set_cursor_line_style(Style::default());
    if has_focus {
        textarea.set_selection_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg),
        );
    }
    let want_ln = if show_line_numbers {
        Some(Style::default().fg(theme.muted))
    } else {
        None
    };
    if textarea.line_number_style() != want_ln {
        match want_ln {
            Some(s) => textarea.set_line_number_style(s),
            None => textarea.remove_line_number(),
        }
    }
    frame.render_widget(&*textarea, area);
}

/// Highlight search-match cells in the rendered frame buffer.
/// Walks each visible row, reconstructs the grapheme string, and paints
/// the background of every cell that falls within a case-insensitive match.
pub fn overlay_search_highlights(frame: &mut Frame, app: &App, area: Rect) {
    let Some(popup) = app.editor.find_popup.as_ref() else {
        return;
    };
    let query = popup.query();
    if query.is_empty() {
        return;
    }
    let ql = query.to_lowercase();
    let editor = &app.editor.editor;
    let inner = editor.block().map(|b| b.inner(area)).unwrap_or(area);
    let gutter = if app.editor.show_line_numbers {
        editor.lines().len().to_string().len() as u16 + 2
    } else {
        0
    };
    let bg = app.app_theme.highlight_bg;
    let buf = frame.buffer_mut();
    let content_left = inner.left() + gutter;
    for y in inner.top()..inner.bottom() {
        let mut graphemes: Vec<(u16, u16)> = Vec::new();
        let mut concat = String::new();
        let mut starts: Vec<usize> = Vec::new();
        for x in content_left..inner.right() {
            let Some(cell) = buf.cell((x, y)) else {
                continue;
            };
            let sym = cell.symbol();
            if sym.is_empty() {
                if let Some((_, w)) = graphemes.last_mut() {
                    *w = w.saturating_add(1);
                }
            } else {
                starts.push(concat.len());
                concat.push_str(sym);
                graphemes.push((x, 1));
            }
        }
        let lower = concat.to_lowercase();
        let mut from = 0usize;
        while let Some(rel) = lower[from..].find(&ql) {
            let s = from + rel;
            let e = s + ql.len();
            from = e;
            let gi = starts.iter().rposition(|&st| st <= s).unwrap_or(0);
            let gj = starts.iter().rposition(|&st| st < e).unwrap_or(gi);
            let x_start = graphemes[gi].0;
            let x_end = graphemes[gj].0 + graphemes[gj].1;
            for x in x_start..x_end {
                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_bg(bg);
                }
            }
        }
    }
}

/// Overlay EDIT-mode markdown highlighting on top of the rendered textarea.
/// Uses a per-source-line cache rebuilt only when the document changes.
/// Skips the cursor cell (it carries `REVERSED` modifier) and any cell inside
/// an active text selection (non-default background).
pub fn overlay_markdown_highlight(frame: &mut Frame, app: &mut App, area: Rect) {
    let show_ln = app.editor.show_line_numbers;
    let gutter = if show_ln {
        app.editor.editor.lines().len().to_string().len() as u16 + 2
    } else {
        0
    };
    let inner = app
        .editor
        .editor
        .block()
        .map(|b| b.inner(area))
        .unwrap_or(area);

    // --- Phase 1: rebuild per-source-line style cache only when the doc changed.
    // Disjoint borrows of app.editor subfields: &editor.lines() + &mut source_highlighter
    // + &mut cache fields are distinct fields through one &mut binding — allowed by the
    // borrow checker. Zero clones: highlight_line reads full_doc by &[String].
    {
        let e = &mut app.editor;
        let full_doc: &[String] = e.editor.lines();
        let stale =
            e.md_highlight_lines != full_doc.len() || e.md_highlight_change != e.last_editor_change;
        if stale && show_ln {
            let hl = e
                .source_highlighter
                .get_or_insert_with(|| crate::markdown::SourceHighlighter::new(&app.app_theme));
            let mut cache = Vec::with_capacity(full_doc.len());
            for (i, line) in full_doc.iter().enumerate() {
                cache.push(hl.highlight_line(line, i, full_doc));
            }
            e.md_highlight_cache = cache;
            e.md_highlight_lines = full_doc.len();
            e.md_highlight_change = e.last_editor_change;
        }
    }

    let buf = frame.buffer_mut();
    let content_left = inner.left() + gutter;

    if show_ln {
        let cache = &app.editor.md_highlight_cache;
        let mut source_idx: Option<usize> = None;
        let mut rows_for_line: Vec<(u16, u16, u16)> = Vec::new();

        for y in inner.top()..inner.bottom() {
            let gutter_text: String = (inner.x..content_left)
                .filter_map(|x| buf.cell((x, y)).map(|c| c.symbol()))
                .collect();
            let trimmed_gutter = gutter_text.trim();
            if trimmed_gutter.is_empty() {
                if source_idx.is_some() {
                    if let Some(last) = rows_for_line.last_mut() {
                        last.2 = inner.right();
                    } else {
                        rows_for_line.push((y, content_left, inner.right()));
                    }
                }
            } else if let Ok(n) = trimmed_gutter.parse::<usize>() {
                if let Some(si) = source_idx {
                    let styles = cache.get(si).map(Vec::as_slice).unwrap_or(&[]);
                    apply_highlight_styles(buf, &rows_for_line, styles);
                }
                source_idx = Some(n.saturating_sub(1));
                rows_for_line = vec![(y, content_left, inner.right())];
            }
        }
        if let Some(si) = source_idx {
            let styles = cache.get(si).map(Vec::as_slice).unwrap_or(&[]);
            apply_highlight_styles(buf, &rows_for_line, styles);
        }
    } else {
        // No line numbers: source_idx is unknown, so highlight the displayed text
        // standalone per frame. Rare path (line numbers default on); left as-is.
        let e = &mut app.editor;
        let full_doc: Vec<String> = e.editor.lines().to_vec();
        let hl = e
            .source_highlighter
            .get_or_insert_with(|| crate::markdown::SourceHighlighter::new(&app.app_theme));
        for y in inner.top()..inner.bottom() {
            let displayed: String = (content_left..inner.right())
                .filter_map(|x| buf.cell((x, y)).map(|c| c.symbol()))
                .collect();
            if displayed.is_empty() {
                continue;
            }
            let styles = hl.highlight_line(&displayed, 0, &full_doc);
            let mut ci = 0usize;
            for x in content_left..inner.right() {
                if ci >= styles.len() {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    if cell.symbol().is_empty() {
                        continue;
                    }
                    if cell.modifier.contains(ratatui::style::Modifier::REVERSED) {
                        ci += 1;
                        continue;
                    }
                    if cell.bg != app.app_theme.bg.unwrap_or(ratatui::style::Color::Reset) {
                        ci += 1;
                        continue;
                    }
                    cell.set_style(styles[ci]);
                    ci += 1;
                }
            }
        }
    }
}

/// Apply precomputed highlight styles for a group of rows corresponding to one source line.
fn apply_highlight_styles(
    buf: &mut ratatui::prelude::Buffer,
    rows: &[(u16, u16, u16)],
    styles: &[ratatui::style::Style],
) {
    if styles.is_empty() {
        return;
    }
    let mut src_cursor = 0usize;
    for &(y, x_start, x_end) in rows {
        for x in x_start..x_end {
            let Some(cell) = buf.cell_mut((x, y)) else {
                continue;
            };
            let sym = cell.symbol();
            if sym.is_empty() {
                continue;
            }
            if cell.modifier.contains(ratatui::style::Modifier::REVERSED) {
                src_cursor += 1;
                continue;
            }
            if cell.bg != ratatui::style::Color::Reset {
                src_cursor += 1;
                continue;
            }
            if src_cursor >= styles.len() {
                break;
            }
            cell.set_style(styles[src_cursor]);
            src_cursor += 1;
        }
    }
}
