use anyhow::{Context, Result};
use ratatui::{prelude::*, widgets::*};
use ratatui_textarea::TextArea;
use std::path::Path;
use std::process::Command;

use crate::app::{App, EditFocus, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::overlay::OverlayView;

mod edit_view;
mod help;
pub(crate) mod help_content;
mod list_view;
mod popups;
pub(crate) mod quick_search;
pub(crate) mod setup;
mod title_bar;

pub use edit_view::draw_edit_view;
pub use help::*;
pub use help_content::{HelpSuggestion, roll_suggestions};
pub(crate) use list_view::{
    draw_list_view, get_preview_info, list_detail_line, list_view_layout, section_rects,
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
    if app.popups.active.is_some() {
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
            if let Some(graf) = &mut app.graph_state {
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
                    ctx.app_status = Some(app.status.as_ref());
                    ctx.vault_path = Some(&app.storage.data_dir);
                    ctx.date_format = Some(&app.date_format);
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
                    );
                }

                graf.overlay_render(frame, outer[1], &app.app_theme, &app.config, None);
            }
        }

        ViewMode::Draw => {
            if let Some(draw) = &mut app.draw_state {
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
                let spans = build_tab_spans(&tabs, active, hovered, &app.app_theme, false, icon_mode);
                let mut ctx =
                    crate::statusline::StatuslineContext::for_overlay(&app.config, ViewMode::Draw);
                ctx.area = Some(outer[0]);
                ctx.app_status = Some(app.status.as_ref());
                ctx.vault_path = Some(&app.storage.data_dir);
                ctx.date_format = Some(&app.date_format);
                ctx.draw = Some(draw);
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
                );
                draw.mouse_pos = app.mouse_pos;
                draw.overlay_render(frame, outer[1], &app.app_theme, &app.config, None);
            }
        }
        ViewMode::Canvas => {
            if let Some(canvas) = &mut app.canvas_state {
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
                ctx.canvas = Some(canvas);
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
                );
                canvas.overlay_render(frame, outer[1], &app.app_theme, &app.config, None);
            }
        }
        ViewMode::Backup => {
            if let Some(backup) = &mut app.backup_state {
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
                    backup,
                    app.config.ui.icon_mode,
                    app.mouse_pos,
                );
                backup.mouse_pos = app.mouse_pos;
                backup.overlay_render(frame, outer[1], &app.app_theme, &app.config, None);
            }
        }
        ViewMode::ContentTree => {
            if let Some(tree) = &mut app.content_tree_state {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                let mut ctx = crate::statusline::StatuslineContext::for_overlay(
                    &app.config,
                    ViewMode::ContentTree,
                );
                ctx.area = Some(outer[0]);
                ctx.app_status = Some(app.status.as_ref());
                ctx.vault_path = Some(&app.storage.data_dir);
                ctx.date_format = Some(&app.date_format);
                ctx.content_tree = Some(tree);
                let (left_line, right_line) = crate::statusline::render_header(
                    &ctx,
                    &app.config.statusline,
                    ViewMode::ContentTree,
                    &app.app_theme,
                );
                draw_view_title_bar(
                    frame,
                    outer[0],
                    &app.app_theme,
                    left_line,
                    right_line,
                    Some(app.status.as_ref()),
                );
                tree.mouse_pos = app.mouse_pos;
                tree.overlay_render(frame, outer[1], &app.app_theme, &app.config, None);
            }
        }
    }
    // Restore mouse_pos for popup hover detection
    app.mouse_pos = popup_hover_pos;

    // Global popups — rendered on top of the active view
    // Template popup
    if let Some(crate::popups::ActivePopup::Template(popup)) = &app.popups.active {
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

        crate::ui::render_list_with_selection(
            frame,
            tags_list,
            chunks[1],
            (popup.focus == crate::popups::TagPopupFocus::AllTagsList
                && !popup.all_tags.is_empty())
            .then_some(popup.all_tags_selected),
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

        let hovered_idx = app.mouse_pos.and_then(|(col, row)| {
            let inner_y = chunks[1].y + 1;
            if !picker.filtered_folders.is_empty()
                && row >= inner_y
                && row < inner_y + picker.filtered_folders.len() as u16
                && col >= chunks[1].x + 1
                && col < chunks[1].x + chunks[1].width - 1
            {
                Some((row - inner_y) as usize)
            } else {
                None
            }
        });

        let items: Vec<ListItem> = if picker.filtered_folders.is_empty() {
            crate::ui::empty_list_item(&app.app_theme, "(no matching folders)")
        } else {
            picker
                .filtered_folders
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let label = if f.is_empty() { "Vault (Root)" } else { f };
                    let mut item = ListItem::new(label);
                    if Some(i) == hovered_idx {
                        item = item.style(app.app_theme.hover_style());
                    }
                    item
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

        crate::ui::render_list_with_selection(
            frame,
            list,
            chunks[1],
            (picker.focus == crate::app::FolderPickerFocus::Results
                && !picker.filtered_folders.is_empty())
            .then_some(picker.selected),
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
                && col >= chunks[2].x + 1
                && col < chunks[2].x + chunks[2].width - 1
            {
                let scroll_offset = palette.state.offset();
                let idx = ((row - inner_y) / 2) as usize + scroll_offset;
                if idx < palette.items.len() {
                    Some(idx)
                } else {
                    None
                }
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
            let grep_hovered = app.mouse_pos.and_then(|(col, row)| {
                let inner_y = results_chunk.y + 1;
                if !visible.is_empty()
                    && row >= inner_y
                    && row < inner_y + visible.len() as u16
                    && col >= results_chunk.x + 1
                    && col < results_chunk.x + results_chunk.width - 1
                {
                    Some((row - inner_y) as usize)
                } else {
                    None
                }
            });
            let items: Vec<ListItem> = visible
                .iter()
                .enumerate()
                .map(|(vi, (_, t))| {
                    let mut item = ListItem::new(crate::ui::styled_result_line(
                        t,
                        &app.app_theme,
                        app.config.ui.icon_mode,
                    ));
                    if Some(vi) == grep_hovered {
                        item = item.style(app.app_theme.hover_style());
                    }
                    item
                })
                .collect();
            (items, "")
        } else if has_title {
            let title_hovered = app.mouse_pos.and_then(|(col, row)| {
                let inner_y = results_chunk.y + 1;
                if !popup.title_results.is_empty()
                    && row >= inner_y
                    && row < inner_y + popup.title_results.len() as u16
                    && col >= results_chunk.x + 1
                    && col < results_chunk.x + results_chunk.width - 1
                {
                    Some((row - inner_y) as usize)
                } else {
                    None
                }
            });
            let items: Vec<ListItem> = popup
                .title_results
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let mut item = ListItem::new(crate::ui::styled_result_line(
                        entry,
                        &app.app_theme,
                        app.config.ui.icon_mode,
                    ));
                    if Some(i) == title_hovered {
                        item = item.style(app.app_theme.hover_style());
                    }
                    item
                })
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

    // Trash view popup
    if let Some(crate::popups::ActivePopup::TrashView(trash)) = &app.popups.active {
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

        let hovered_idx = app.mouse_pos.and_then(|(col, row)| {
            let inner_y = content.y + 1;
            if !trash.items.is_empty()
                && row >= inner_y
                && row < inner_y + trash.items.len() as u16
                && col >= content.x + 1
                && col < content.x + content.width - 1
            {
                Some((row - inner_y) as usize)
            } else {
                None
            }
        });

        let items: Vec<ListItem> = trash
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let name = item.name.to_string_lossy();
                let when = crate::ui::format_relative_time(item.time_deleted as u64);
                let mut list_item = ListItem::new(Line::from(vec![
                    Span::raw(name.to_string()),
                    Span::styled(
                        format!("  ({when})"),
                        Style::default().fg(app.app_theme.muted),
                    ),
                ]));
                if Some(i) == hovered_idx {
                    list_item = list_item.style(app.app_theme.hover_style());
                }
                list_item
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

        crate::ui::render_list_with_selection(frame, list, content, Some(trash.selected));
    }

    // Confirm popup
    if let Some(popup) = &app.popups.confirm {
        draw_confirm_popup(frame, popup, frame.area(), &app.app_theme);
    }

    if let Some(popup) = &app.popups.active {
    popup.draw(frame, frame.area(), &app.app_theme, &app.keybinds, app.mouse_pos);
    }

    // Context menu (from edit view)
    if let Some(crate::popups::ActivePopup::ContextMenu(menu)) = &app.popups.active {
        let labels = [" Copy ", " Cut ", " Paste ", " Select All "];
        let menu_width = labels.iter().map(|l| l.len() as u16).max().unwrap_or(0);
        let menu_height = labels.len() as u16;

        let x = menu.x.min(frame.area().width.saturating_sub(menu_width));
        let y = menu.y.min(frame.area().height.saturating_sub(menu_height));
        let menu_area = Rect::new(x, y, menu_width, menu_height);

        let hovered_idx = app.mouse_pos.and_then(|(col, row)| {
            if col >= x && col < x + menu_width && row >= y && row < y + menu_height {
                Some((row - y) as usize)
            } else {
                None
            }
        });

        let items: Vec<ListItem> = labels.iter().enumerate().map(|(i, l)| {
            let mut item = ListItem::new(*l);
            if Some(i) == hovered_idx {
                item = item.style(app.app_theme.hover_style());
            }
            item
        }).collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.preview_bg_style())
                    .borders(Borders::NONE),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_widget(Clear, menu_area);
        crate::ui::render_list_with_selection(frame, list, menu_area, Some(menu.selected));
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

pub fn get_textarea_scroll(textarea: &TextArea) -> (usize, usize) {
    let mut scroll_row = 0;
    let mut scroll_col = 0;

    let debug_str = format!("{textarea:?}");
    if let Some(start) = debug_str.find("viewport: Viewport(") {
        let after_start = &debug_str[start + "viewport: Viewport(".len()..];
        if let Some(end) = after_start.find(')') {
            let number_str = &after_start[..end];
            if let Ok(number) = number_str.parse::<u64>() {
                scroll_row = ((number >> 16) & 0xFFFF) as usize;
                scroll_col = (number & 0xFFFF) as usize;
            }
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
