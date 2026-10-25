use anyhow::{Context, Result};
use ratatui::{prelude::*, widgets::*};
use ratatui_textarea::{TextArea, WrapMode};
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

use crate::app::{App, EditFocus, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::overlay::OverlayView;

pub(crate) mod braille;
pub(crate) mod chrome;
mod edit_view;
mod footer;
pub(crate) use footer::content_area as view_content_area;
mod help;
pub(crate) mod help_content;
pub mod icons;
mod list_view;
pub(crate) mod message_overlay;
mod popups;
pub(crate) mod quick_keybinds;
pub(crate) mod quick_search;
pub(crate) mod scrollbar;
pub(crate) mod setup;
mod title_bar;
pub(crate) mod vault_switcher;

pub use edit_view::draw_edit_view;
pub use help::*;
pub use help_content::{HelpSuggestion, roll_suggestions};
pub(crate) use list_view::{
    GridTileSpec, browser_columns, draw_list_view, get_preview_info, grid_dims, grid_tile_rect,
    list_view_layout, render_grid_tile, section_rects,
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
        IconMode::Emoji => icons::emoji_for_pair(nerd, unicode),
        IconMode::None => "",
    }
}

/// Return the appropriate icon character based on `mode`.
pub fn get_char(nerd: char, unicode: char, mode: IconMode) -> char {
    match mode {
        IconMode::Nerd => nerd,
        IconMode::Unicode | IconMode::Emoji => unicode,
        IconMode::None => ' ',
    }
}

pub(crate) fn note_file_glyph(
    note_id: &str,
    theme: &AppThemeColors,
    mode: IconMode,
) -> (char, &'static str, Color) {
    let ext = Path::new(note_id)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    let (icon, label, color) = match ext.to_ascii_lowercase().as_str() {
        "md" => (icons::MARKDOWN, "MD", theme.text),
        "txt" => (icons::FILE, "TXT", theme.text),
        "clin" => (icons::LOCK, "CX", theme.destructive),
        "draw" => (icons::DRAW, "D", theme.success),
        "canvas" => (icons::CANVAS, "C", theme.accent),
        "base" => (icons::BASES, "BASE", theme.accent),
        "png" => (icons::PNG, "PNG", theme.warning),
        "jpg" | "jpeg" => (icons::JPEG, "JPEG", theme.warning),
        "gif" => (icons::GIF, "GIF", theme.warning),
        "webp" => (icons::WEBP, "WEBP", theme.warning),
        _ => (icons::UNKNOWN, "?", theme.text),
    };
    (icon.char(mode), label, color)
}

pub(crate) fn note_file_icon(
    note_id: &str,
    theme: &AppThemeColors,
    mode: IconMode,
) -> Option<Span<'static>> {
    let (icon, _, color) = note_file_glyph(note_id, theme, mode);
    (mode != IconMode::None).then(|| {
        Span::styled(
            format!("{icon} "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )
    })
}

pub fn tab_vec_from_array<'a>(arr: &[(&'a str, &'a str)]) -> Vec<(&'a str, Option<&'a str>)> {
    arr.iter().map(|&(l, g)| (l, Some(g))).collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PopupSize {
    Small,        // 40% width, 40% height. Max bounds: 60 cols x 20 rows
    Medium,       // 50% width, 50% height. Max bounds: 80 cols x 30 rows
    Large,        // 60% width, 60% height. Max bounds: 100 cols x 40 rows
    Prompt,       // 50% width. Fixed 5 height. Max bounds: 80 cols wide
    Confirm,      // 50% width. Fixed 12 height. Max bounds: 80 cols wide
    Content(u16), // 50% width, bounded content height.
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewHeaderInfo {
    pub path: String,
    pub item_name: String,
    pub prev_name: Option<String>,
    pub next_name: Option<String>,
}

fn background_images_visible(app: &App) -> bool {
    // Kitty consumes its first transmission even if a later overlay erases it.
    app.popups.active.is_none()
        && app.popups.confirm.is_none()
        && app.command_palette.is_none()
        && app.vault_switcher.is_none()
        && !app.quick_keybinds_open
        && !app.base_embeds.has_popup()
        && !app.messages.is_active()
        && (app.mode != ViewMode::Edit
            || (app.editor.find_popup.is_none()
                && app.editor.go_to_line_input.is_none()
                && !app.editor.link_preview
                && !app.status.starts_with("Save failed")
                && !app.status.starts_with("Could not open")))
}

pub fn draw_ui(frame: &mut Frame, app: &mut App, focus: EditFocus) {
    app.header_tabs.clear();
    app.refresh_command_palette();
    app.base_embeds.begin_frame();
    app.list.image_cache.begin_frame();
    app.editor.image_cache.begin_frame();
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

    let view_area = view_content_area(frame.area());
    if app.mode != ViewMode::Setup && !(app.mode == ViewMode::Edit && app.zen_mode) {
        let header = chrome::rows(frame.area())[0];
        let mut ctx = crate::statusline::StatuslineContext::for_view(app, app.mode);
        ctx.area = Some(header);
        let (left, right) = crate::statusline::render_header(
            &ctx,
            &app.config.statusline,
            app.mode,
            &app.app_theme,
        );
        draw_view_title_bar(
            frame,
            header,
            &app.app_theme,
            left,
            right,
            Some(app.status.as_ref()),
            app.load_spinner_tick,
        );
    }
    if app.mode != ViewMode::Setup && (view_area.height == 0 || view_area.width == 0) {
        app.mouse_pos = popup_hover_pos;
        app.list.image_cache.end_frame();
        app.editor.image_cache.end_frame();
        footer::draw(frame, app);
        return;
    }
    match app.mode {
        ViewMode::List => draw_list_view(frame, app),
        ViewMode::Edit => draw_edit_view(frame, app, focus),
        ViewMode::Help => draw_help_view(frame, app),
        ViewMode::Setup => draw_setup_view(frame, app),
        ViewMode::Calendar => {
            if let Some(mut calendar) = app.calendar_view.take() {
                calendar.overlay_render(frame, view_area, app);
                app.calendar_view = Some(calendar);
            }
        }
        ViewMode::Graph => {
            if let Some(mut graf) = app.graph_plugin.take() {
                let outer = crate::ui::chrome::content_rows(view_area);

                let banner = graf
                    .graph_state
                    .as_ref()
                    .and_then(|gs| gs.read().mode_banner);
                if let Some(mode) = banner {
                    // Cover the header bar, exactly like Notes SELECT MODE.
                    let text: &'static str = match mode {
                        graf::ModeBanner::CreateConnection => {
                            " CONNECTION MODE \u{2014} select target "
                        }
                        graf::ModeBanner::DeleteConnection => {
                            " DELETE CONNECTION MODE \u{2014} select target "
                        }
                        graf::ModeBanner::LocalGraph => " LOCAL GRAPH ONLY ",
                        graf::ModeBanner::GroupedGraph => " GROUPED GRAPH ONLY ",
                        graf::ModeBanner::BoxSelect => " BOX SELECT \u{2014} drag, release ",
                    };
                    let header_rect = outer[0];
                    frame.render_widget(Clear, header_rect);
                    frame.render_widget(
                        Block::default().style(Style::default().bg(app.app_theme.accent)),
                        header_rect,
                    );
                    let w = text.chars().count() as u16;
                    let x = header_rect.x + (header_rect.width.saturating_sub(w)) / 2;
                    frame.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            text,
                            Style::default()
                                .fg(app.app_theme.highlight_fg)
                                .bg(app.app_theme.accent)
                                .add_modifier(Modifier::BOLD),
                        ))),
                        Rect::new(x, header_rect.y, w, 1),
                    );
                } else {
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
                    ctx.graph_grid_visible = graf.grid;
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
                app.graph_plugin = Some(graf);
            }
        }

        ViewMode::Draw => {
            if let Some(mut draw) = app.draw_state.take() {
                let outer = crate::ui::chrome::content_rows(view_area);
                let icon_mode = app.config.ui.icon_mode;
                let tabs_arr = crate::draw_adapter::draw_tool_tabs(icon_mode);
                let tabs = tab_vec_from_array(&tabs_arr);
                let active = crate::draw_adapter::draw_tool_tab_index(draw.state.active_tool);
                draw.sync_header_status(app);
                let hovered = None;
                let spans =
                    build_tab_spans(&tabs, active, hovered, &app.app_theme, false, icon_mode);
                let mut ctx =
                    crate::statusline::StatuslineContext::for_overlay(&app.config, ViewMode::Draw);
                ctx.area = Some(outer[0]);
                ctx.app_status = Some(app.status.as_ref());
                ctx.vault_path = Some(&app.storage.data_dir);
                ctx.date_format = Some(&app.date_format);
                ctx.draw = Some(&draw.state);
                let (left_line, right_line) = crate::statusline::render_header(
                    &ctx,
                    &app.config.statusline,
                    ViewMode::Draw,
                    &app.app_theme,
                );
                app.header_tabs = crate::ui::draw_header_tabs(
                    frame,
                    outer[0],
                    &app.app_theme,
                    left_line,
                    spans,
                    active,
                    right_line,
                    Some(app.status.as_ref()),
                    app.load_spinner_tick,
                    app.mouse_pos,
                );
                draw.state.mouse_pos = app.mouse_pos;
                draw.overlay_render(frame, outer[1], app);
                app.draw_state = Some(draw);
            }
        }
        ViewMode::Canvas => {
            if let Some(mut canvas) = app.canvas_state.take() {
                let outer = crate::ui::chrome::content_rows(view_area);
                let mut ctx = crate::statusline::StatuslineContext::for_overlay(
                    &app.config,
                    ViewMode::Canvas,
                );
                ctx.area = Some(outer[0]);
                ctx.app_status = Some(app.status.as_ref());
                ctx.vault_path = Some(&app.storage.data_dir);
                ctx.date_format = Some(&app.date_format);
                ctx.canvas = Some(&canvas.state);
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
        ViewMode::Bases => {
            if let Some(mut bases) = app.bases_state.take() {
                bases.overlay_render(frame, view_area, app);
                app.bases_state = Some(bases);
            }
        }

        ViewMode::Outline => {
            if let Some(mut tree) = app.outline_state.take() {
                let outer = crate::ui::chrome::content_rows(view_area);
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
    app.list.image_cache.end_frame();
    app.editor.image_cache.end_frame();
    footer::draw(frame, app);
    // Restore mouse_pos for popup hover detection
    app.mouse_pos = popup_hover_pos;

    app.base_embeds.draw_popup(frame, app);
    crate::ui::quick_keybinds::draw_quick_keybinds(frame, app);
    if app.messages.is_active() {
        crate::ui::message_overlay::draw_message_overlay(frame, app, &app.app_theme, frame.area());
    }
    crate::ui::vault_switcher::draw_vault_switcher(frame, app);

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
        draw_text_prompt_popup(
            frame,
            title,
            PopupSize::Prompt,
            PopupHints::Keybinds(&text_input_hints("confirm")),
            &mut popup.input,
            Style::default().fg(app.app_theme.heading),
            &app.app_theme,
        );
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
        frame.render_widget(input_block, input_chunks[0]);
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
        let track = crate::ui::scrollbar::pane_track(
            inner_tags,
            Some(crate::ui::scrollbar::track_rect(chunks[1])),
        );
        popup.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track,
            content_len: popup.all_tags.len(),
            viewport_len: inner_tags.height as usize,
        });
        crate::ui::scrollbar::draw_scrollbar(
            frame,
            track,
            popup.all_tags.len(),
            inner_tags.height as usize,
            popup.all_tags_selected,
            popup.all_tags.len().saturating_sub(1),
            &app.app_theme,
        );
    }

    // RemoveTags popup
    if let Some(crate::popups::ActivePopup::RemoveTags(popup)) = &mut app.popups.active {
        let content = draw_popup_frame(
            frame,
            frame.area(),
            "REMOVE TAGS",
            PopupSize::Large,
            PopupHints::Keybinds(&[
                ("j/k".to_string(), "move"),
                ("Space".to_string(), "toggle"),
                ("a".to_string(), "all"),
                ("Enter".to_string(), "remove"),
                ("d".to_string(), "remove all"),
                ("Esc".to_string(), "cancel"),
            ]),
            &app.app_theme,
        );
        let total = popup.total_selected;
        let tag_count = popup.tags.len();
        let items: Vec<ListItem> = if tag_count == 0 {
            crate::ui::empty_list_item(&app.app_theme, "No tags to remove")
        } else {
            popup
                .tags
                .iter()
                .enumerate()
                .map(|(i, tag)| {
                    let count = popup.tag_counts.get(i).copied().unwrap_or(0);
                    let count_label = if count >= total {
                        "(all)"
                    } else {
                        &format!("({count})")
                    };
                    let is_selected = popup.selected.contains(&i);
                    let label = format!(
                        "{} {tag} {count_label}",
                        if is_selected { "[x]" } else { "[ ]" }
                    );
                    let style = if is_selected {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Line::from(label)).style(style)
                })
                .collect()
        };

        let tags_list = build_list_widget(items, &app.app_theme)
            .block(
                Block::default()
                    .style(app.app_theme.bg_style())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(app.app_theme.heading)),
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
            content,
            (!popup.tags.is_empty()).then_some(popup.cursor),
            popup.scroll_offset,
        );
        popup.scroll_offset = state.offset();
        let inner_tags = Rect {
            x: content.x + 1,
            y: content.y + 1,
            width: content.width.saturating_sub(2),
            height: content.height.saturating_sub(2),
        };
        crate::ui::paint_list_hover(
            frame,
            inner_tags,
            &state,
            popup.tags.len(),
            app.mouse_pos,
            app.app_theme.hover_style(),
        );
        let track = crate::ui::scrollbar::pane_track(
            inner_tags,
            Some(crate::ui::scrollbar::track_rect(content)),
        );
        popup.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track,
            content_len: popup.tags.len(),
            viewport_len: inner_tags.height as usize,
        });
        crate::ui::scrollbar::draw_scrollbar(
            frame,
            track,
            popup.tags.len(),
            inner_tags.height as usize,
            popup.cursor,
            popup.tags.len().saturating_sub(1),
            &app.app_theme,
        );

        if let Some(confirm) = &mut popup.confirm {
            draw_confirm_popup(frame, confirm, frame.area(), &app.app_theme, true, None);
        }
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
            crate::ui::empty_list_item(&app.app_theme, "No matching folders")
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
        let track = crate::ui::scrollbar::pane_track(
            inner,
            Some(crate::ui::scrollbar::track_rect(chunks[1])),
        );
        picker.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track,
            content_len: picker.filtered_folders.len(),
            viewport_len: inner.height as usize,
        });
        crate::ui::scrollbar::draw_scrollbar(
            frame,
            track,
            picker.filtered_folders.len(),
            inner.height as usize,
            picker.selected,
            picker.filtered_folders.len().saturating_sub(1),
            &app.app_theme,
        );
    }

    let p_tabs = if app.command_palette.is_some() {
        Some(crate::palette::palette_tabs(app))
    } else {
        None
    };
    let workflow_bar = app
        .command_palette
        .as_ref()
        .map(|p| p.workflow_bar(app))
        .unwrap_or_default();
    // Command palette; separate Bases confirmations render in their original view.
    if !app.palette_bases_dialog_open()
        && let Some(palette) = &mut app.command_palette
    {
        let area = frame.area();
        let mut hints = if palette.workflow.is_none() {
            vec![
                ("Tab".to_string(), "category"),
                ("Enter".to_string(), "run"),
                ("↑/↓".to_string(), "select"),
                ("Esc".to_string(), "close"),
            ]
        } else {
            vec![
                ("Enter".to_string(), "select/apply"),
                ("↑/↓".to_string(), "move"),
                ("Esc".to_string(), "back"),
            ]
        };
        if let Some(crate::palette::workflows::Workflow::Theme(popup, _)) = &palette.workflow {
            hints[0].1 = "apply";
            hints.push(("Alt+B".to_string(), "bg"));
            if !popup.setup_owned {
                hints.push(("Alt+G".to_string(), "graph"));
            }
        }
        if matches!(
            palette.workflow,
            Some(crate::palette::workflows::Workflow::Features)
        ) {
            hints[0].1 = "toggle";
        }
        let content = draw_popup_frame(
            frame,
            area,
            palette.workflow_title(),
            palette.popup_size(area, !workflow_bar.is_empty()),
            PopupHints::Keybinds(&hints),
            &app.app_theme,
        );

        let chunks = palette.content_areas(content, !workflow_bar.is_empty());

        palette.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(
                    Style::default().fg(
                        if palette
                            .bases_page()
                            .is_some_and(crate::bases::palette::Page::is_destructive)
                        {
                            app.app_theme.destructive
                        } else {
                            app.app_theme.heading
                        },
                    ),
                )
                .title(if palette.is_value_input() {
                    "Value"
                } else if palette.workflow.is_some() {
                    "Filter choices"
                } else {
                    ""
                }),
        );
        frame.render_widget(&palette.input, chunks[0]);

        if palette.workflow.is_none() {
            let tabs: Vec<(&str, Option<&str>)> = p_tabs
                .clone()
                .expect("palette is some so p_tabs is some")
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
        } else if !workflow_bar.is_empty() {
            let labels: Vec<_> = workflow_bar
                .iter()
                .map(|(line, _)| line.to_string())
                .collect();
            let tabs: Vec<_> = labels.iter().map(|label| (label.as_str(), None)).collect();
            let hovered = app.mouse_pos.and_then(|(col, row)| {
                (row == chunks[1].y)
                    .then(|| {
                        hit_test_tabs(
                            &tabs,
                            chunks[1].x,
                            chunks[1].width,
                            chunks[1].x,
                            col,
                            false,
                            crate::config::IconMode::None,
                        )
                        .filter(|&i| workflow_bar[i].1.is_some())
                    })
                    .flatten()
            });
            let mut spans = Vec::new();
            for (i, (line, _)) in workflow_bar.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw(" "));
                }
                let mut style = Style::default()
                    .fg(app.app_theme.accent)
                    .add_modifier(Modifier::BOLD);
                if hovered == Some(i) {
                    style = style.patch(app.app_theme.hover_style());
                }
                spans.push(Span::styled(" ", style));
                spans.extend(line.spans.iter().cloned().map(|mut span| {
                    span.style = style.patch(span.style);
                    span
                }));
                spans.push(Span::styled(" ", style));
            }
            frame.render_widget(
                Paragraph::new(Line::from(spans))
                    .alignment(Alignment::Center)
                    .style(app.app_theme.hint_line_bg_style()),
                chunks[1],
            );
        }
        frame.render_widget(
            Paragraph::new(palette.workflow_hint())
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(if palette.error.is_some() {
                    app.app_theme.destructive
                } else {
                    app.app_theme.muted
                })),
            chunks[2],
        );
        let hovered_cmd_idx = app.mouse_pos.and_then(|(col, row)| {
            let inner_y = chunks[3].y + 1;
            if !palette.items.is_empty()
                && row >= inner_y
                && col > chunks[3].x
                && col < chunks[3].x + chunks[3].width - 1
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
                let selected = palette.state.selected() == Some(i);
                let mut preview = palette.item_preview(item, &app.app_theme);
                if selected {
                    for span in &mut spans {
                        span.style.fg = Some(app.app_theme.highlight_fg);
                    }
                    if let Some(description) = preview.spans.last_mut() {
                        description.style.fg = Some(app.app_theme.highlight_fg);
                    }
                }
                let mut list_item = ListItem::new(vec![Line::from(spans), preview]);
                if selected {
                    list_item = list_item.style(app.app_theme.selected_style());
                } else if Some(i) == hovered_cmd_idx {
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
                    .border_style(
                        Style::default().fg(
                            if palette
                                .bases_page()
                                .is_some_and(crate::bases::palette::Page::is_destructive)
                            {
                                app.app_theme.destructive
                            } else {
                                app.app_theme.heading
                            },
                        ),
                    )
                    .title(""),
            )
            .highlight_style(Style::default())
            .highlight_symbol("  ");

        frame.render_stateful_widget(list, chunks[3], &mut palette.state);
        let content_len = palette.items.len();
        if content_len > 0 {
            let viewport_len = ((chunks[3].height.saturating_sub(2)) / 2) as usize;
            let meta = crate::ui::scrollbar::ScrollbarMeta {
                track: crate::ui::scrollbar::pane_track(
                    Block::bordered().inner(chunks[3]),
                    Some(crate::ui::scrollbar::track_rect(chunks[3])),
                ),
                content_len,
                viewport_len,
            };
            palette.last_scroll = Some(meta);
            palette.last_results_area = Some(chunks[3]);
            if app.config.ui.scrollbars {
                let pos = palette.state.selected().unwrap_or(0);
                crate::ui::scrollbar::draw_scrollbar(
                    frame,
                    meta.track,
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
        let title = popup.input.lines().join("");
        let stem = app.storage.note_file_stem_from_title(&title);
        let ext = Path::new(&popup.note_id)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        let target = app.storage.unique_note_id(&stem, ext, &popup.note_id);
        let preview = if title.trim().is_empty() {
            "Name cannot be empty".to_owned()
        } else {
            format!(
                "Will rename to: {}",
                crate::fsutil::sanitize_for_terminal(&target)
            )
        };
        draw_filename_prompt_popup(
            frame,
            "RENAME FILE",
            &mut popup.input,
            &preview,
            PopupHints::Text("Enter rename · Esc cancel · Links are not updated"),
            &app.app_theme,
        );
    }

    // Vault rename popup
    if let Some(crate::popups::ActivePopup::VaultRename(popup)) = &mut app.popups.active {
        draw_text_prompt_popup(
            frame,
            "RENAME VAULT",
            PopupSize::Prompt,
            PopupHints::Keybinds(&text_input_hints("rename")),
            &mut popup.input,
            Style::default().fg(app.app_theme.heading),
            &app.app_theme,
        );
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
        draw_text_prompt_popup(
            frame,
            title,
            PopupSize::Prompt,
            PopupHints::Keybinds(&keybinds),
            &mut popup.input,
            Style::default().fg(app.app_theme.heading),
            &app.app_theme,
        );
    }

    // Create note popup
    if let Some(crate::popups::ActivePopup::CreateNote(popup, format)) = &mut app.popups.active {
        let title = match format {
            crate::popups::NoteFormat::Markdown => "NEW NOTE",
            crate::popups::NoteFormat::Draw => "NEW DRAWING",
            crate::popups::NoteFormat::Canvas => "NEW CANVAS",
            crate::popups::NoteFormat::PlainText => "NEW TEXT FILE",
            crate::popups::NoteFormat::Base => "NEW BASES FILE",
        };
        if matches!(
            format,
            crate::popups::NoteFormat::Markdown | crate::popups::NoteFormat::PlainText
        ) {
            let (ext, default) = if *format == crate::popups::NoteFormat::PlainText {
                ("txt", "Untitled text")
            } else {
                ("md", "Untitled note")
            };
            let name = popup.input.lines().join("");
            let name = if name.trim().is_empty() {
                default
            } else {
                name.trim()
            };
            let stem = app.storage.note_file_stem_from_title(name);
            let initial = Path::new(&popup.folder).join(app.storage.new_note_id());
            let target = app
                .storage
                .unique_note_id(&stem, ext, &initial.to_string_lossy());
            let preview = format!(
                "Will create: {}",
                crate::fsutil::sanitize_for_terminal(&target)
            );
            draw_filename_prompt_popup(
                frame,
                title,
                &mut popup.input,
                &preview,
                PopupHints::Keybinds(&text_input_hints("create")),
                &app.app_theme,
            );
        } else {
            draw_text_prompt_popup(
                frame,
                title,
                PopupSize::Prompt,
                PopupHints::Keybinds(&text_input_hints("create")),
                &mut popup.input,
                Style::default().fg(app.app_theme.heading),
                &app.app_theme,
            );
        }
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
        draw_text_prompt_popup(
            frame,
            title,
            PopupSize::Prompt,
            PopupHints::Keybinds(&text_input_hints("import")),
            &mut popup.input,
            Style::default().fg(app.app_theme.heading),
            &app.app_theme,
        );
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
                ("sn:".to_string(), "subnotes"),
                ("\\e\\".to_string(), "escapes filters"),
            ]),
            &app.app_theme,
        );

        let query_text = popup.input.lines().join("");
        let parsed = crate::app::parse_search_query(
            &query_text,
            app.config.features.tags.is_enabled(),
            app.config.features.subnotes.is_enabled(),
        );
        let has_filter = parsed.folder_filter.is_some()
            || parsed.pinned_only
            || parsed.tag_filter.is_some()
            || parsed.grep_mode
            || parsed.subnote_text.is_some();

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
                    crate::ui::icons::OPEN_FOLDER
                        .get(app.config.ui.icon_mode)
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
                let pin_icon = crate::ui::icons::PIN.get(app.config.ui.icon_mode);
                spans.push(Span::styled(
                    format!("{pin_icon} Pinned"),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if let Some(sn_text) = &parsed.subnote_text {
                add_sep(&mut spans, &mut first, &app.app_theme);
                let sn_display = if sn_text.is_empty() {
                    "Subnotes".to_string()
                } else {
                    sn_text.clone()
                };
                let sn_icon = crate::ui::icons::SUBNOTES.get(app.config.ui.icon_mode);
                spans.push(Span::styled(
                    format!("{sn_icon} {sn_display}"),
                    Style::default()
                        .fg(app.app_theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
            } else if parsed.grep_mode {
                add_sep(&mut spans, &mut first, &app.app_theme);
                let grep_display = if parsed.grep_text.is_empty() {
                    "Grep".to_string()
                } else {
                    parsed.grep_text.clone()
                };
                let search_icon = crate::ui::icons::SEARCH.get(app.config.ui.icon_mode);
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
                    crate::ui::icons::TAG
                        .get(app.config.ui.icon_mode)
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
        let has_subnote = !popup.subnote_results.is_empty();
        let has_grep = parsed.grep_mode && parsed.subnote_text.is_none();

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
        } else if has_subnote {
            popup.subnote_results.len()
        } else if has_title {
            popup.title_result_ids.len()
        } else {
            0
        };

        let selected_idx = if has_grep {
            popup.grep_selected
        } else if has_subnote {
            popup.subnote_selected
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
                                "▾ "
                            } else {
                                "▸ "
                            };
                            let note_summary =
                                app.notes.iter().find(|n| n.id.as_str() == &*hit.note_id);
                            let title = note_summary
                                .map(crate::storage::NoteSummary::file_stem)
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
                            let mut line = crate::ui::styled_result_line(
                                &header_text,
                                &app.app_theme,
                                app.config.ui.icon_mode,
                            );
                            if let Some(icon) = note_file_icon(
                                &hit.note_id,
                                &app.app_theme,
                                app.config.ui.icon_mode,
                            ) {
                                line.spans.insert(0, icon);
                            }
                            ListItem::new(line)
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
        } else if has_subnote {
            (offset..end)
                .map(|idx| {
                    let hit = &popup.subnote_results[idx];
                    let mut line = crate::ui::styled_result_line(
                        &hit.label,
                        &app.app_theme,
                        app.config.ui.icon_mode,
                    );
                    if let Some(icon) =
                        note_file_icon(&hit.parent_id, &app.app_theme, app.config.ui.icon_mode)
                    {
                        line.spans.insert(0, icon);
                    }
                    ListItem::new(line)
                })
                .collect()
        } else if has_title {
            (offset..end)
                .map(|idx| {
                    let id_arc = &popup.title_result_ids[idx];
                    let note_summary = app.notes.iter().find(|n| n.id.as_str() == &**id_arc);
                    let title = note_summary
                        .map(crate::storage::NoteSummary::file_stem)
                        .unwrap_or(&**id_arc);
                    let folder = note_summary.map(|n| n.folder.as_str()).unwrap_or("");
                    let label = if folder.is_empty() {
                        title.to_string()
                    } else {
                        format!("{folder}/{title}")
                    };
                    let mut line = crate::ui::styled_result_line(
                        &label,
                        &app.app_theme,
                        app.config.ui.icon_mode,
                    );
                    if let Some(icon) =
                        note_file_icon(id_arc, &app.app_theme, app.config.ui.icon_mode)
                    {
                        line.spans.insert(0, icon);
                    }
                    ListItem::new(line)
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
        let track = crate::ui::scrollbar::pane_track(
            inner_results,
            Some(crate::ui::scrollbar::track_rect(results_chunk)),
        );
        popup.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track,
            content_len: total_items,
            viewport_len,
        });
        if app.config.ui.scrollbars {
            crate::ui::scrollbar::draw_scrollbar(
                frame,
                track,
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

        let items: Vec<ListItem> = if trash.items.is_empty() {
            empty_list_item(&app.app_theme, "Trash is empty")
        } else {
            trash
                .items
                .iter()
                .map(|item| {
                    let name = item.name.to_string_lossy();
                    let when = crate::statusline::list_relative_age(item.time_deleted as u64);
                    ListItem::new(Line::from(vec![
                        Span::raw(name.to_string()),
                        Span::styled(
                            format!("  ({when})"),
                            Style::default().fg(app.app_theme.muted),
                        ),
                    ]))
                })
                .collect()
        };

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
            (!trash.items.is_empty()).then_some(trash.selected),
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
        let track = crate::ui::scrollbar::pane_track(
            inner,
            Some(crate::ui::scrollbar::track_rect(content)),
        );
        trash.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track,
            content_len: trash.items.len(),
            viewport_len: inner.height as usize,
        });
        crate::ui::scrollbar::draw_scrollbar(
            frame,
            track,
            trash.items.len(),
            inner.height as usize,
            trash.selected,
            trash.items.len().saturating_sub(1),
            &app.app_theme,
        );
    }

    // Confirm popup
    if let Some(popup) = &mut app.popups.confirm {
        let literal_yes_no =
            !matches!(&app.popups.active, Some(crate::popups::ActivePopup::Tag(_)));
        draw_confirm_popup(
            frame,
            popup,
            frame.area(),
            &app.app_theme,
            literal_yes_no,
            app.mouse_pos,
        );
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
}

fn draw_filename_prompt_popup(
    frame: &mut Frame,
    title: &str,
    input: &mut TextArea,
    preview: &str,
    hints: PopupHints<'_>,
    theme: &AppThemeColors,
) {
    let content = draw_popup_frame(frame, frame.area(), title, PopupSize::Prompt, hints, theme);
    input.set_block(
        Block::default()
            .style(theme.bg_style())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.heading))
            .title_bottom(Line::from(Span::styled(
                format!(" {preview} "),
                Style::default().fg(theme.muted),
            ))),
    );
    frame.render_widget(&*input, content);
}

fn draw_text_prompt_popup(
    frame: &mut Frame,
    title: &str,
    size: PopupSize,
    hints: PopupHints<'_>,
    input: &mut TextArea,
    border_style: Style,
    theme: &AppThemeColors,
) {
    let content = draw_popup_frame(frame, frame.area(), title, size, hints, theme);
    input.set_block(
        Block::default()
            .style(theme.bg_style())
            .borders(Borders::ALL)
            .border_style(border_style),
    );
    frame.render_widget(&*input, content);
}

fn spawn_opener(command: &str, args: &[&OsStr], path: &Path) -> Result<()> {
    let mut cmd = Command::new(command);
    cmd.args(args)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to launch {command}"))?;
    Ok(())
}

pub fn open_in_file_manager(path: &Path) -> Result<()> {
    let command = if cfg!(target_os = "linux") {
        "xdg-open"
    } else if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        anyhow::bail!("opening file manager is not supported on this platform")
    };
    spawn_opener(command, &[], path)
}

pub fn open_with_default_application(path: &Path) -> Result<()> {
    let command = if cfg!(target_os = "linux") {
        "xdg-open"
    } else if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "cmd"
    } else {
        anyhow::bail!("opening files is not supported on this platform")
    };
    if cfg!(target_os = "windows") {
        // `cmd /C start "" <path>` invokes the associated application.
        spawn_opener(
            command,
            &[OsStr::new("/C"), OsStr::new("start"), OsStr::new("")],
            path,
        )
    } else {
        spawn_opener(command, &[], path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryPickerOutcome {
    Selected(std::path::PathBuf),
    Cancelled,
    Unavailable,
}

/// Open a native directory picker without making a platform helper mandatory.
pub fn pick_directory(prompt: &str) -> Result<DirectoryPickerOutcome> {
    fn selected(output: std::process::Output) -> Result<DirectoryPickerOutcome> {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            return Ok(if path.is_empty() {
                DirectoryPickerOutcome::Cancelled
            } else {
                DirectoryPickerOutcome::Selected(path.into())
            });
        }
        if matches!(output.status.code(), Some(1)) {
            return Ok(DirectoryPickerOutcome::Cancelled);
        }
        anyhow::bail!("directory picker exited with {}", output.status);
    }

    if cfg!(target_os = "linux") {
        if crate::fsutil::can_run("zenity") {
            return selected(
                Command::new("zenity")
                    .args([
                        "--file-selection",
                        "--directory",
                        &format!("--title={prompt}"),
                    ])
                    .output()
                    .context("failed to launch zenity")?,
            );
        }
        if crate::fsutil::can_run("kdialog") {
            return selected(
                Command::new("kdialog")
                    .args(["--getexistingdirectory", "."])
                    .output()
                    .context("failed to launch kdialog")?,
            );
        }
        return Ok(DirectoryPickerOutcome::Unavailable);
    }
    if cfg!(target_os = "macos") {
        return selected(
            Command::new("osascript")
                .args([
                    "-e",
                    &format!("POSIX path of (choose folder with prompt \"{prompt}\")"),
                ])
                .output()
                .context("failed to launch osascript")?,
        );
    }
    if cfg!(target_os = "windows") {
        let script = "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.FolderBrowserDialog; if ($f.ShowDialog() -eq 'OK') { $f.SelectedPath }";
        return selected(
            Command::new("powershell")
                .args(["-Command", script])
                .output()
                .context("failed to launch PowerShell")?,
        );
    }
    Ok(DirectoryPickerOutcome::Unavailable)
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
        if crate::fsutil::can_run("zenity") {
            let output = Command::new("zenity")
                .arg("--file-selection")
                .arg(format!("--file-filter={filter_name} | {zenity_glob}"))
                .output()?;
            if output.status.success() {
                return Ok(Some(
                    String::from_utf8_lossy(&output.stdout).trim().to_string(),
                ));
            }
        } else if crate::fsutil::can_run("kdialog") {
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

pub(crate) fn refresh_editor_document_viewport(
    document: &crate::editor_document::EditorDocument,
    prev_row: u16,
    prev_col: u16,
    area: Rect,
    line_numbers: bool,
) -> (u16, u16) {
    refresh_textarea_viewport(document.textarea(), prev_row, prev_col, area, line_numbers)
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

pub(crate) fn render_editor_document_with_theme(
    frame: &mut Frame,
    document: &mut crate::editor_document::EditorDocument,
    area: Rect,
    theme: &AppThemeColors,
    has_focus: bool,
    show_line_numbers: bool,
    block: Block<'static>,
    base_style: Style,
) {
    render_textarea_with_theme(
        frame,
        document.textarea_mut(),
        area,
        theme,
        has_focus,
        show_line_numbers,
        block,
        base_style,
    );
}

/// Rewrap changed source lines rather than scanning the whole document per key.
pub(crate) fn apply_editor_visual_change(
    editor: &mut crate::editor::NoteEditor,
    change: &crate::editor_document::DocumentChange,
) {
    use crate::editor_document::DocumentChange;
    let DocumentChange::Lines { old, new } = change else {
        editor.visual_row_cache.key = None;
        return;
    };
    let Some((revision, width, numbers, mode, tabs)) = editor.visual_row_cache.key else {
        return;
    };
    if revision == editor.body.revision() {
        return;
    }
    let source = editor.body.lines();
    let old_count = editor
        .visual_row_cache
        .rows
        .last()
        .map_or(0, |row| row.source_line + 1);
    if old.start != new.start
        || old.end > old_count
        || new.end > source.len()
        || old_count - old.len() + new.len() != source.len()
        || mode != editor.body.textarea().wrap_mode()
        || tabs != editor.body.textarea().tab_length()
        || editor_wrap_width(width, old_count, numbers)
            != editor_wrap_width(width, source.len(), numbers)
    {
        editor.visual_row_cache.key = None;
        return;
    }
    let first = editor
        .visual_row_cache
        .rows
        .partition_point(|row| row.source_line < old.start);
    let last = editor
        .visual_row_cache
        .rows
        .partition_point(|row| row.source_line < old.end);
    let wrap_width = editor_wrap_width(width, source.len(), numbers);
    let mut replacement = Vec::new();
    for (offset, line) in source[new.clone()].iter().enumerate() {
        for (start, end) in editor_line_ranges(line, mode, wrap_width, tabs) {
            replacement.push(crate::editor::EditorVisualRow {
                source_line: new.start + offset,
                start_char: line[..start].chars().count(),
                end_char: line[..end].chars().count(),
            });
        }
    }
    let suffix = first + replacement.len();
    editor
        .visual_row_cache
        .rows
        .splice(first..last, replacement);
    if old.len() != new.len() {
        for row in &mut editor.visual_row_cache.rows[suffix..] {
            row.source_line = row.source_line - old.len() + new.len();
        }
    }
    editor.visual_row_cache.key = Some((editor.body.revision(), width, numbers, mode, tabs));
}

pub(crate) fn ensure_editor_visual_rows(
    editor: &mut crate::editor::NoteEditor,
    inner_width: u16,
    show_ln: bool,
) {
    use ratatui_textarea::WrapMode;
    let wrap_mode = editor.body.textarea().wrap_mode();
    let tab_len = editor.body.textarea().tab_length();

    if wrap_mode != WrapMode::None {
        let key = (
            editor.body.revision(),
            inner_width,
            show_ln,
            wrap_mode,
            tab_len,
        );
        if editor.visual_row_cache.key != Some(key) {
            let full_doc: &[String] = editor.body.lines();
            editor.visual_row_cache.rows =
                editor_visual_rows(full_doc, wrap_mode, inner_width, show_ln, tab_len);
            editor.visual_row_cache.key = Some(key);
        }
    } else {
        // Screen rows are source rows with wrapping disabled. Never reuse the
        // old wrapped map after toggling soft wrap.
        editor.visual_row_cache.key = None;
        editor.visual_row_cache.rows.clear();
    }
}

/// Shift rendered editor content for center/right/justified alignment.
/// Operates on `frame.buffer_mut()` after the textarea has been rendered left-aligned.
pub(crate) fn overlay_text_alignment(frame: &mut Frame, app: &mut App, area: Rect) {
    use crate::config::TextAlignment;
    use ratatui_textarea::WrapMode;

    let align = app.editor.text_align;
    if align == TextAlignment::Left {
        return;
    }
    // Only meaningful when wrap is on.
    if app.editor.body.textarea().wrap_mode() == WrapMode::None {
        return;
    }

    let inner = app.editor.body.inner_rect(area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let gutter: u16 = if app.editor_show_line_numbers() {
        app.editor.body.lines().len().max(1).to_string().len() as u16 + 2
    } else {
        0
    };

    let text_left = inner.x + gutter;
    let text_width = inner.width.saturating_sub(gutter);
    if text_width == 0 {
        return;
    }

    let show_ln = app.editor_show_line_numbers();
    ensure_editor_visual_rows(&mut app.editor, inner.width, show_ln);

    let rows = &app.editor.visual_row_cache.rows;
    let scroll_top = app.editor.body_viewport_row as usize;
    let buf = frame.buffer_mut();

    for screen_row in 0..inner.height {
        let y = inner.y + screen_row;
        if y >= buf.area.y + buf.area.height {
            break;
        }

        // Find content width: scan from right to find last non-space symbol.
        let mut content_width: u16 = 0;
        for cx in (0..text_width).rev() {
            let x = text_left + cx;
            if buf[(x, y)].symbol() != " " {
                content_width = cx + 1;
                break;
            }
        }

        if content_width == 0 || content_width >= text_width {
            continue;
        }

        let slack = text_width - content_width;
        let visual_idx = scroll_top + screen_row as usize;

        match align {
            TextAlignment::Center => {
                let pad = slack / 2;
                if pad > 0 {
                    shift_row_right(buf, y, text_left, text_width, content_width, pad);
                }
            }
            TextAlignment::Right => {
                if slack > 0 {
                    shift_row_right(buf, y, text_left, text_width, content_width, slack);
                }
            }
            TextAlignment::Justified => {
                let is_last_of_source = visual_idx >= rows.len()
                    || visual_idx + 1 >= rows.len()
                    || rows.get(visual_idx + 1).map(|r| r.source_line)
                        != rows.get(visual_idx).map(|r| r.source_line);

                if !is_last_of_source && slack > 0 {
                    let mut gap_positions = Vec::new();
                    for cx in 0..content_width {
                        let x = text_left + cx;
                        if buf[(x, y)].symbol() == " " {
                            let has_left = cx > 0 && buf[(text_left + cx - 1, y)].symbol() != " ";
                            let has_right = cx + 1 < content_width
                                && buf[(text_left + cx + 1, y)].symbol() != " ";
                            if has_left && has_right {
                                gap_positions.push(cx);
                            }
                        }
                    }

                    if !gap_positions.is_empty() {
                        let extra = slack as usize;
                        let g = gap_positions.len();
                        let mut gap_idx = 0;
                        let mut new_cells = Vec::with_capacity(text_width as usize);

                        for cx in 0..content_width {
                            let src_cell = buf[(text_left + cx, y)].clone();
                            new_cells.push(src_cell.clone());
                            if gap_idx < g && cx == gap_positions[gap_idx] {
                                let add = extra / g + if gap_idx < extra % g { 1 } else { 0 };
                                for _ in 0..add {
                                    let mut blank = src_cell.clone();
                                    blank.modifier.remove(ratatui::style::Modifier::REVERSED);
                                    new_cells.push(blank);
                                }
                                gap_idx += 1;
                            }
                        }

                        // Clear the row with filler to avoid ghost cursors
                        let filler_x = text_left + text_width - 1;
                        let mut filler = ratatui::buffer::Cell::default();
                        if filler_x < buf.area.x + buf.area.width {
                            filler = buf[(filler_x, y)].clone();
                            filler.set_symbol(" ");
                        }
                        for cx in 0..text_width {
                            let x = text_left + cx;
                            if x < buf.area.x + buf.area.width {
                                buf[(x, y)] = filler.clone();
                            }
                        }

                        for (i, cell) in new_cells.iter().enumerate() {
                            let x = text_left + i as u16;
                            if x < buf.area.x + buf.area.width {
                                buf[(x, y)] = cell.clone();
                            }
                        }
                    }
                }
            }
            TextAlignment::Left => unreachable!(),
        }
    }
}

/// Shift buffer cells in a row rightward by `pad` columns.
fn shift_row_right(
    buf: &mut ratatui::prelude::Buffer,
    y: u16,
    text_left: u16,
    text_width: u16,
    content_width: u16,
    pad: u16,
) {
    let filler_x = text_left + text_width - 1;
    let mut filler = ratatui::buffer::Cell::default();
    if filler_x < buf.area.x + buf.area.width {
        filler = buf[(filler_x, y)].clone();
        filler.set_symbol(" ");
    }

    // Copy from right to left to avoid overwriting.
    for cx in (0..content_width).rev() {
        let src_x = text_left + cx;
        let dst_x = text_left + cx + pad;
        if dst_x < buf.area.x + buf.area.width {
            let cell = buf[(src_x, y)].clone();
            buf[(dst_x, y)] = cell;
        }
        // Clear source cell with filler to prevent ghost cursor and keep bg.
        if src_x < buf.area.x + buf.area.width {
            buf[(src_x, y)] = filler.clone();
        }
    }
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
    let editor = &app.editor.body;
    let inner = editor.inner_rect(area);
    let gutter = if app.editor_show_line_numbers() {
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
pub(crate) fn zen_focus_range(
    lines: &[String],
    cursor_row: usize,
    unit: crate::config::FocusUnit,
    context: usize,
) -> std::ops::Range<usize> {
    if lines.is_empty() {
        return 0..0;
    }
    let mut lo = cursor_row;
    let mut hi = cursor_row;

    match unit {
        crate::config::FocusUnit::Line => {
            lo = cursor_row.saturating_sub(context);
        }
        crate::config::FocusUnit::Paragraph => {
            if !lines[cursor_row].trim().is_empty() {
                // Expand current block downward.
                while hi + 1 < lines.len() && !lines[hi + 1].trim().is_empty() {
                    hi += 1;
                }
                // Expand current block upward.
                while lo > 0 && !lines[lo - 1].trim().is_empty() {
                    lo -= 1;
                }
            }
            // Extend upward by context blocks.
            for _ in 0..context {
                // Skip blank lines above the current block.
                while lo > 0 && lines[lo - 1].trim().is_empty() {
                    lo -= 1;
                }
                // Consume the non-blank block.
                let mut moved = false;
                while lo > 0 && !lines[lo - 1].trim().is_empty() {
                    lo -= 1;
                    moved = true;
                }
                if !moved && lo == 0 {
                    break;
                }
            }
        }
    }
    lo..hi + 1
}

pub fn overlay_zen_focus(frame: &mut Frame, app: &mut App, area: Rect) {
    if !(app.zen_mode && app.config.editor.zen_focus_dimming) {
        return;
    }
    let show_ln = app.editor_show_line_numbers();
    let editor = &mut app.editor;
    let inner = editor.body.inner_rect(area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let lines = editor.body.lines();
    let cursor_row = editor.body.cursor().row;
    let bright = zen_focus_range(
        lines,
        cursor_row,
        app.config.editor.zen_focus_unit,
        app.config.editor.zen_focus_context,
    );

    let wrap_mode = editor.body.textarea().wrap_mode();

    if wrap_mode != ratatui_textarea::WrapMode::None {
        ensure_editor_visual_rows(editor, inner.width, show_ln);
    }

    let rows = &editor.visual_row_cache.rows;
    let viewport_row = editor.body_viewport_row as usize;
    let buf = frame.buffer_mut();

    for i in 0..inner.height {
        let y = inner.y + i;
        if y >= buf.area.y + buf.area.height {
            break;
        }
        let source_line = if wrap_mode != ratatui_textarea::WrapMode::None {
            if viewport_row + i as usize >= rows.len() {
                continue;
            }
            rows[viewport_row + i as usize].source_line
        } else {
            viewport_row + i as usize
        };

        if !bright.contains(&source_line) {
            for x in inner.x..inner.x + inner.width {
                if x < buf.area.x + buf.area.width {
                    let _ = buf.cell_mut((x, y)).map(|c| {
                        c.set_fg(app.app_theme.muted);
                        c.set_bg(app.app_theme.bg.unwrap_or(ratatui::style::Color::Reset));
                        c.modifier = ratatui::style::Modifier::DIM;
                    });
                }
            }
        }
    }
}

pub(crate) fn aligned_cursor_target(
    lines: &[String],
    rows: &[crate::editor::EditorVisualRow],
    viewport_row: u16,
    screen_row: u16,
    rel_col: u16,
    text_width: u16,
    align: crate::config::TextAlignment,
    tab_len: u8,
) -> Option<(usize, usize)> {
    use crate::config::TextAlignment;
    use unicode_width::UnicodeWidthChar;

    let visual_idx = viewport_row as usize + screen_row as usize;
    if visual_idx >= rows.len() {
        let last = rows.last()?;
        return Some((last.source_line, last.end_char));
    }
    let row = rows[visual_idx];
    let line = lines.get(row.source_line)?;

    let mut display_cells = Vec::new(); // (cell_idx, char_idx)
    let mut display_width = 0usize;

    for (offset, ch) in line
        .chars()
        .skip(row.start_char)
        .take(row.end_char.saturating_sub(row.start_char))
        .enumerate()
    {
        let width = if ch == '\t' {
            if tab_len == 0 {
                0
            } else {
                let tab = usize::from(tab_len);
                tab - (display_width % tab)
            }
        } else {
            ch.width().unwrap_or(0)
        };

        for _ in 0..width {
            display_cells.push((display_width, offset));
            display_width += 1;
        }
    }

    // Find content width (exclude trailing space cells)
    let mut content_width = 0;
    for (cell_idx, char_idx) in display_cells.iter().rev() {
        let ch = line.chars().nth(row.start_char + char_idx).unwrap_or(' ');
        if ch != ' ' {
            content_width = *cell_idx + 1;
            break;
        }
    }

    let content_width = (content_width as u16).min(text_width);
    let slack = text_width.saturating_sub(content_width);

    if content_width == 0 || slack == 0 {
        let matched = display_cells.iter().find(|(cx, _)| *cx == rel_col as usize);
        return Some((
            row.source_line,
            row.start_char
                + matched
                    .map(|(_, i)| *i)
                    .unwrap_or(row.end_char.saturating_sub(row.start_char)),
        ));
    }

    let mut adjusted_col = rel_col as i32;

    match align {
        TextAlignment::Center => {
            adjusted_col -= (slack / 2) as i32;
        }
        TextAlignment::Right => {
            adjusted_col -= slack as i32;
        }
        TextAlignment::Justified => {
            let is_last_of_source =
                visual_idx + 1 >= rows.len() || rows[visual_idx + 1].source_line != row.source_line;
            if !is_last_of_source {
                let mut gap_positions = Vec::new();
                for cx in 0..content_width {
                    let ch = display_cells
                        .iter()
                        .find(|(idx, _)| *idx == cx as usize)
                        .map(|(_, i)| line.chars().nth(row.start_char + *i).unwrap_or(' '))
                        .unwrap_or(' ');
                    if ch == ' ' {
                        let left_ch = if cx > 0 {
                            display_cells
                                .iter()
                                .find(|(idx, _)| *idx == (cx - 1) as usize)
                                .map(|(_, i)| line.chars().nth(row.start_char + *i).unwrap_or(' '))
                                .unwrap_or(' ')
                        } else {
                            ' '
                        };
                        let right_ch = if cx + 1 < content_width {
                            display_cells
                                .iter()
                                .find(|(idx, _)| *idx == (cx + 1) as usize)
                                .map(|(_, i)| line.chars().nth(row.start_char + *i).unwrap_or(' '))
                                .unwrap_or(' ')
                        } else {
                            ' '
                        };
                        if left_ch != ' ' && right_ch != ' ' {
                            gap_positions.push(cx);
                        }
                    }
                }

                if !gap_positions.is_empty() {
                    let extra = slack as usize;
                    let g = gap_positions.len();
                    let mut gap_idx = 0;

                    let mut shifts = std::collections::BTreeMap::new();
                    for cx in 0..content_width {
                        if gap_idx < g && cx == gap_positions[gap_idx] {
                            let add = extra / g + if gap_idx < extra % g { 1 } else { 0 };
                            shifts.insert(cx, add);
                            gap_idx += 1;
                        }
                    }

                    let mut visual_x = 0;
                    let mut found = false;
                    for cx in 0..content_width {
                        if visual_x == rel_col as usize {
                            adjusted_col = cx as i32;
                            found = true;
                            break;
                        }
                        visual_x += 1;
                        if let Some(&add) = shifts.get(&cx) {
                            if (rel_col as usize) < visual_x + add {
                                adjusted_col = cx as i32;
                                found = true;
                                break;
                            }
                            visual_x += add;
                        }
                    }
                    if !found {
                        adjusted_col = content_width as i32;
                    }
                }
            }
        }
        TextAlignment::Left => {}
    }

    if adjusted_col < 0 {
        return Some((row.source_line, row.start_char));
    }

    let matched = display_cells
        .iter()
        .find(|(cx, _)| *cx == adjusted_col as usize);
    let char_offset = matched
        .map(|(_, i)| *i)
        .unwrap_or(row.end_char.saturating_sub(row.start_char));
    Some((row.source_line, row.start_char + char_offset))
}

/// Overlay EDIT-mode markdown highlighting on top of the rendered textarea.
/// Uses a per-source-line cache rebuilt only when the document changes.
/// Mirror ratatui-textarea 0.9.2's wrapped-row map using source character offsets.
pub(crate) fn editor_visual_rows(
    lines: &[String],
    mode: WrapMode,
    total_width: u16,
    line_numbers: bool,
    tab_len: u8,
) -> Vec<crate::editor::EditorVisualRow> {
    let width = editor_wrap_width(total_width, lines.len(), line_numbers);
    let mut rows = Vec::new();
    for (source_line, line) in lines.iter().enumerate() {
        for (start_byte, end_byte) in editor_line_ranges(line, mode, width, tab_len) {
            rows.push(crate::editor::EditorVisualRow {
                source_line,
                start_char: line[..start_byte].chars().count(),
                end_char: line[..end_byte].chars().count(),
            });
        }
    }
    rows
}

fn editor_wrap_width(total_width: u16, line_count: usize, line_numbers: bool) -> usize {
    let reserved = if line_numbers {
        line_count.max(1).to_string().len() + 2
    } else {
        0
    };
    usize::from(total_width).saturating_sub(reserved).max(1)
}

#[cfg(test)]
mod visual_row_cache_tests;

fn editor_line_ranges(
    line: &str,
    mode: WrapMode,
    width: usize,
    tab_len: u8,
) -> Vec<(usize, usize)> {
    if mode == WrapMode::None {
        return vec![(0, line.len())];
    }
    let width = width.max(1);
    let mut out = match mode {
        WrapMode::None => vec![(0, line.len())],
        WrapMode::Glyph => {
            let mut chunks = Vec::new();
            split_editor_range_by_grapheme_width(line, 0, line.len(), width, tab_len, &mut chunks);
            chunks
        }
        WrapMode::Word => editor_word_chunks(line, width, tab_len, false),
        WrapMode::WordOrGlyph => editor_word_chunks(line, width, tab_len, true),
    };
    if out.is_empty() {
        out.push((0, 0));
    }
    out
}

fn editor_word_chunks(
    line: &str,
    width: usize,
    tab_len: u8,
    fallback_to_glyph: bool,
) -> Vec<(usize, usize)> {
    let chunks: Vec<_> = UnicodeSegmentation::split_word_bound_indices(line)
        .map(|(start, text)| (start, start + text.len()))
        .collect();
    if chunks.is_empty() {
        return vec![(0, 0)];
    }
    let mut out = Vec::new();
    let mut index = 0;
    let mut segment_start = chunks[0].0;
    let mut segment_end = segment_start;
    let mut segment_width = 0;
    while index < chunks.len() {
        let (start, end) = chunks[index];
        if segment_end == segment_start {
            segment_start = start;
        }
        let chunk_width = editor_display_width_from(&line[start..end], segment_width, tab_len);
        if segment_width + chunk_width <= width {
            segment_end = end;
            segment_width += chunk_width;
            index += 1;
            continue;
        }
        if segment_end > segment_start {
            out.push((segment_start, segment_end));
            segment_start = segment_end;
            segment_width = 0;
            continue;
        }
        if fallback_to_glyph {
            split_editor_range_by_grapheme_width(line, start, end, width, tab_len, &mut out);
        } else {
            out.push((start, end));
        }
        index += 1;
        segment_start = end;
        segment_end = end;
        segment_width = 0;
    }
    if segment_end > segment_start {
        out.push((segment_start, segment_end));
    }
    out
}

fn split_editor_range_by_grapheme_width(
    line: &str,
    start: usize,
    end: usize,
    width: usize,
    tab_len: u8,
    out: &mut Vec<(usize, usize)>,
) {
    let mut segment_start = start;
    while segment_start < end {
        let mut segment_end = segment_start;
        let mut segment_width = 0;
        for (offset, grapheme) in
            UnicodeSegmentation::grapheme_indices(&line[segment_start..end], true)
        {
            let grapheme_start = segment_start + offset;
            let grapheme_end = grapheme_start + grapheme.len();
            let next_width = editor_display_width_to(grapheme, segment_width, tab_len);
            let grapheme_width = next_width.saturating_sub(segment_width);
            if segment_end != segment_start && segment_width + grapheme_width > width {
                break;
            }
            segment_end = grapheme_end;
            segment_width = next_width;
            if segment_width > width {
                break;
            }
        }
        if segment_end == segment_start {
            if let Some(ch) = line[segment_start..end].chars().next() {
                segment_end = segment_start + ch.len_utf8();
            } else {
                break;
            }
        }
        out.push((segment_start, segment_end));
        segment_start = segment_end;
    }
}

fn editor_display_width_from(text: &str, start_width: usize, tab_len: u8) -> usize {
    editor_display_width_to(text, start_width, tab_len).saturating_sub(start_width)
}

fn editor_display_width_to(text: &str, mut width: usize, tab_len: u8) -> usize {
    for ch in text.chars() {
        if ch == '\t' {
            if tab_len > 0 {
                let tab = usize::from(tab_len);
                width += tab - (width % tab);
            }
        } else {
            width += ch.width().unwrap_or(0);
        }
    }
    width
}

/// Overlay EDIT-mode markdown highlighting on top of rendered textarea.
pub fn overlay_markdown_highlight(frame: &mut Frame, app: &mut App, area: Rect) {
    let is_todo_txt = app
        .editor
        .editing_id
        .as_ref()
        .is_some_and(|id| id.ends_with("todo.txt"));
    let show_ln = app.editor_show_line_numbers();
    let gutter = if show_ln {
        app.editor.body.lines().len().to_string().len() as u16 + 2
    } else {
        0
    };
    let inner = app.editor.body.inner_rect(area);
    let wrap_mode = app.editor.body.textarea().wrap_mode();
    let tab_len = app.editor.body.textarea().tab_length();

    let e = &mut app.editor;
    if let Some(change) = e.body.take_change() {
        apply_editor_visual_change(e, &change);
        if let Some(hl) = &mut e.source_highlighter {
            hl.apply_change(&e.body, change);
        }
    }
    if e.source_highlighter.as_ref().is_none_or(|hl| {
        !hl.matches_settings(
            &app.app_theme,
            app.config.editor.ghost_syntax,
            app.config.editor.extended_markdown_features,
        )
    }) {
        let mut hl = crate::markdown::SourceHighlighter::new(
            &app.app_theme,
            app.config.editor.ghost_syntax,
            app.config.editor.extended_markdown_features,
        );
        hl.rescan(e.body.lines());
        e.source_highlighter = Some(hl);
        e.md_highlight_memo.clear();
    }
    let capacity = usize::from(inner.height).saturating_mul(8).clamp(256, 2048);
    if e.md_highlight_memo.cap().get() != capacity {
        e.md_highlight_memo
            .resize(std::num::NonZeroUsize::new(capacity).expect("clamped capacity"));
    }
    if wrap_mode != WrapMode::None {
        ensure_editor_visual_rows(e, inner.width, show_ln);
    }
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let horizontal_scroll = if wrap_mode == WrapMode::None {
        usize::from(e.body_viewport_col)
    } else {
        0
    };
    let content_left = inner
        .left()
        .saturating_add(gutter.saturating_sub(horizontal_scroll as u16));
    if content_left >= inner.right() {
        return;
    }
    let base_bg = app.app_theme.bg.unwrap_or(ratatui::style::Color::Reset);
    let visible_start = usize::from(e.body_viewport_row);
    let hl = e
        .source_highlighter
        .as_mut()
        .expect("initialized highlighter");
    let buf = frame.buffer_mut();
    for offset in 0..usize::from(inner.height) {
        let row = if wrap_mode == WrapMode::None {
            let source_line = visible_start + offset;
            let Some(line) = e.body.lines().get(source_line) else {
                break;
            };
            crate::editor::EditorVisualRow {
                source_line,
                start_char: 0,
                end_char: line.chars().count(),
            }
        } else {
            let Some(row) = e.visual_row_cache.rows.get(visible_start + offset) else {
                break;
            };
            *row
        };
        let Some(line) = e.body.lines().get(row.source_line) else {
            continue;
        };
        let key = (line.clone(), hl.is_code_line(row.source_line), is_todo_txt);
        let styles = e.md_highlight_memo.get_or_insert(key, || {
            hl.highlight_line(line, row.source_line, is_todo_txt).into()
        });
        apply_visual_row_highlight(
            buf,
            inner.top().saturating_add(offset as u16),
            content_left,
            inner.right(),
            line,
            row.start_char,
            row.end_char,
            styles,
            tab_len,
            horizontal_scroll.saturating_sub(usize::from(gutter)),
            base_bg,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_visual_row_highlight(
    buf: &mut ratatui::prelude::Buffer,
    y: u16,
    x_start: u16,
    x_end: u16,
    source: &str,
    start_char: usize,
    end_char: usize,
    styles: &[ratatui::style::Style],
    tab_len: u8,
    horizontal_scroll: usize,
    base_bg: ratatui::style::Color,
) {
    if start_char >= end_char || styles.is_empty() {
        return;
    }
    let mut x = x_start;
    let mut display_width = 0usize;
    for (offset, ch) in source
        .chars()
        .skip(start_char)
        .take(end_char.saturating_sub(start_char))
        .enumerate()
    {
        let style_index = start_char + offset;
        let Some(style) = styles.get(style_index) else {
            break;
        };
        let width = if ch == '\t' {
            if tab_len == 0 {
                0
            } else {
                let tab = usize::from(tab_len);
                tab - (display_width % tab)
            }
        } else {
            ch.width().unwrap_or(0)
        };
        for column in display_width..display_width + width {
            if column < horizontal_scroll {
                continue;
            }
            if x >= x_end {
                return;
            }
            if let Some(cell) = buf.cell_mut((x, y))
                && !cell.modifier.contains(ratatui::style::Modifier::REVERSED)
                && cell.bg == base_bg
            {
                cell.set_style(*style);
            }
            x = x.saturating_add(1);
        }
        display_width += width;
    }
}

#[cfg(test)]
mod markdown_highlight_tests {
    #[test]
    fn preview_source_anchor_tracks_resize_and_wrap_toggle() {
        use ratatui_textarea::WrapMode;
        let mut editor = crate::editor::NoteEditor::new();
        editor
            .body
            .textarea_mut()
            .insert_str("long source line that wraps across the editor width\nnext");
        editor
            .body
            .textarea_mut()
            .set_wrap_mode(WrapMode::WordOrGlyph);
        super::ensure_editor_visual_rows(&mut editor, 12, false);
        assert_eq!(editor.visual_row_cache.source_position(1), (0, 1));
        super::ensure_editor_visual_rows(&mut editor, 100, false);
        assert_eq!(editor.visual_row_cache.source_position(1), (1, 0));
        editor.body.textarea_mut().set_wrap_mode(WrapMode::None);
        super::ensure_editor_visual_rows(&mut editor, 12, false);
        assert_eq!(editor.visual_row_cache.source_position(1), (1, 0));
        assert_eq!(editor.visual_row_cache.rows, []);
    }

    use super::*;
    use crate::editor_document::EditorDocument;
    use crate::storage::Storage;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Style;
    use ratatui::widgets::Block;

    fn storage(root: &std::path::Path) -> Storage {
        let data_dir = root.join("data");
        let config_dir = root.join("config");
        let notes_dir = root.join("notes");
        let templates_dir = root.join("templates");
        for path in [&data_dir, &config_dir, &notes_dir, &templates_dir] {
            std::fs::create_dir_all(path).expect("create test directory");
        }
        Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        }
    }

    #[test]
    fn warning_overlay_preserves_first_kitty_image_transmission() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = storage(temp.path());
        let path = storage.notes_dir.join("checker.png");
        let image = image::DynamicImage::new_rgb8(64, 64);
        image.save(&path).expect("image fixture");
        std::fs::write(
            storage.notes_dir.join("note.md"),
            "![checker](checker.png)\n",
        )
        .expect("note fixture");
        let mut app = App::new(storage).expect("app");
        let mut picker = ratatui_image::picker::Picker::halfblocks();
        picker.set_protocol_type(ratatui_image::picker::ProtocolType::Kitty);
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        app.image_picker = Some(picker);
        app.image_decode_tx = Some(tx);
        app.editor.editor_preview_enabled = true;
        app.open_note_at_line("note.md", None);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("terminal");
        terminal
            .draw(|frame| draw_ui(frame, &mut app, EditFocus::Body))
            .expect("layout");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app
            .editor
            .md_preview_renderer
            .as_ref()
            .is_some_and(crate::markdown::MarkdownRenderer::is_pending)
        {
            assert!(
                std::time::Instant::now() < deadline,
                "preview worker timed out"
            );
            app.poll_editor_renderers();
            std::thread::yield_now();
        }
        terminal
            .draw(|frame| draw_ui(frame, &mut app, EditFocus::Body))
            .expect("image request");
        let job = rx.try_recv().expect("image decode request");
        assert_eq!(job.key, path.canonicalize().expect("canonical image path"));
        app.install_image(crate::image_render::worker::DecodedImage {
            key: job.key,
            request: job.request,
            result: Ok(crate::image_render::worker::ImageOutput::Decoded(image)),
        });
        app.messages.push(
            "A startup warning covering the preview".repeat(5),
            crate::app::messages::MessageSeverity::Warning,
        );
        let mut transmitted = false;
        terminal
            .draw(|frame| {
                draw_ui(frame, &mut app, EditFocus::Body);
                transmitted = frame
                    .buffer_mut()
                    .content()
                    .iter()
                    .any(|cell| cell.symbol().contains("\u{1b}_Gq=2,i="));
            })
            .expect("covered frame");
        assert!(
            !transmitted,
            "covered graphics must not consume their transmission"
        );
        app.messages.messages.clear();
        terminal
            .draw(|frame| {
                draw_ui(frame, &mut app, EditFocus::Body);
                transmitted = frame
                    .buffer_mut()
                    .content()
                    .iter()
                    .any(|cell| cell.symbol().contains("\u{1b}_Gq=2,i="));
            })
            .expect("uncovered frame");
        assert!(!transmitted, "encoding must run outside rendering");
        let job = rx.try_recv().expect("visible image encoding request");
        let (completed, responses) = std::sync::mpsc::sync_channel(2);
        crate::image_render::worker::process_job(job, &completed);
        app.install_image(responses.recv().expect("encoded image"));
        terminal
            .draw(|frame| {
                draw_ui(frame, &mut app, EditFocus::Body);
                transmitted = frame
                    .buffer_mut()
                    .content()
                    .iter()
                    .any(|cell| cell.symbol().contains("\u{1b}_Gq=2,i="));
            })
            .expect("encoded frame");
        assert!(
            transmitted,
            "first visible frame must still transmit the image"
        );
    }

    #[test]
    fn aligned_cursor_target_compensates_shift() {
        use crate::config::TextAlignment;
        use crate::editor::EditorVisualRow;
        let lines = vec!["aa bb cc".to_string()];
        // One visual row, text width 20, content 8 → slack 12.
        let rows = vec![EditorVisualRow {
            source_line: 0,
            start_char: 0,
            end_char: 8,
        }];
        // Center: pad 6. Click on 'b' of first "bb" (cells 3..5 → screen 9..11).
        let (l, c) = aligned_cursor_target(&lines, &rows, 0, 0, 9, 20, TextAlignment::Center, 0)
            .expect("target");
        assert_eq!((l, c), (0, 3));
        // Right: pad 12. 'c' of "cc" starts at cell 6 → screen 18.
        let (l, c) = aligned_cursor_target(&lines, &rows, 0, 0, 18, 20, TextAlignment::Right, 0)
            .expect("target");
        assert_eq!((l, c), (0, 6));
        // Left of pad → col 0; past end → last char.
        let (l, c) = aligned_cursor_target(&lines, &rows, 0, 0, 2, 20, TextAlignment::Center, 0)
            .expect("target");
        assert_eq!((l, c), (0, 0));
        let (l, c) = aligned_cursor_target(&lines, &rows, 0, 0, 19, 20, TextAlignment::Right, 0)
            .expect("target");
        assert_eq!((l, c), (0, 7));
    }

    #[test]
    fn right_alignment_leaves_single_cursor() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = crate::app::App::new(storage(temp.path())).expect("app");
        app.editor.body = EditorDocument::from_lines(vec!["short line".to_string()]);
        app.editor.body.set_wrap_mode(WrapMode::WordOrGlyph);
        app.editor.text_align = crate::config::TextAlignment::Right;
        let mut terminal = Terminal::new(TestBackend::new(60, 5)).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                crate::ui::edit_view::render_editor_widget(
                    frame,
                    &mut app,
                    crate::editor::EditFocus::Body,
                    area,
                    None,
                    None,
                );
                overlay_text_alignment(frame, &mut app, area);
            })
            .expect("render");
        let buf = terminal.backend().buffer();
        let reversed = buf
            .content()
            .iter()
            .filter(|c| c.modifier.contains(ratatui::style::Modifier::REVERSED))
            .count();
        assert_eq!(reversed, 1, "exactly one cursor cell after shifting");
    }

    /// Like `display_fragment` but includes the (width-1) blank continuation
    /// cells ratatui writes after each wide grapheme.
    fn display_fragment_cells(fragment: &str, tab_len: u8) -> String {
        let mut out = String::new();
        let mut width = 0usize;
        for ch in fragment.chars() {
            if ch == '\t' {
                let pad = if tab_len == 0 {
                    0
                } else {
                    let tab = usize::from(tab_len);
                    tab - (width % tab)
                };
                width += pad;
                out.push_str(&" ".repeat(pad));
                continue;
            }
            let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            width += w;
            out.push(ch);
            out.push_str(&" ".repeat(w.saturating_sub(1)));
        }
        out
    }

    #[test]
    fn editor_visual_rows_match_textarea_rendering() {
        let lines = vec![
            String::new(),
            "word boundaries stay whole".into(),
            "supercalifragilistic".into(),
            "\tTabbed e\u{301}界".into(),
        ];
        for line_numbers in [false, true] {
            let width: u16 = 12;
            let rows = editor_visual_rows(&lines, WrapMode::WordOrGlyph, width, line_numbers, 4);
            let backend = TestBackend::new(width, rows.len() as u16);
            let mut terminal = Terminal::new(backend).expect("terminal");
            let mut textarea = TextArea::from(lines.clone());
            textarea.set_wrap_mode(WrapMode::WordOrGlyph);
            textarea.set_tab_length(4);
            if line_numbers {
                textarea.set_line_number_style(Style::default());
            }
            terminal
                .draw(|frame| frame.render_widget(&textarea, frame.area()))
                .expect("render");

            let gutter: u16 = if line_numbers { 3 } else { 0 };
            for (y, row) in rows.iter().enumerate() {
                let source = &lines[row.source_line];
                let fragment: String = source
                    .chars()
                    .skip(row.start_char)
                    .take(row.end_char - row.start_char)
                    .collect();
                let actual: String = (gutter..width)
                    .filter_map(|x| {
                        terminal
                            .backend()
                            .buffer()
                            .cell((x, y as u16))
                            .map(|cell| cell.symbol())
                    })
                    .collect();
                assert_eq!(
                    actual.trim_end(),
                    display_fragment_cells(&fragment, 4).trim_end(),
                    "line_numbers={line_numbers}, visual row={y}"
                );
            }
        }
    }

    #[test]
    fn editor_visual_rows_match_textarea_rendering_many_widths() {
        let corpus: Vec<Vec<String>> = vec![
            vec!["# Heading with **bold** and `code` spans".into()],
            vec!["some **bold** text and `code` and a rather long line of words".into()],
            vec![
                "trailing   spaces   inside   and   long   words   like   supercalifragilistic"
                    .into(),
            ],
            vec!["CJK 你好世界 mixed with english words 界界界 and more text here".into()],
            vec!["\tTabbed\ttext\twith\ttabs and words".into()],
            vec!["a  b   c    d     e      f       g        h".into()],
            vec!["- list item with [link](url) and *em* **strong** ***both***".into()],
            vec!["emoji 🎉🎉🎉 clusters and text continue after them".into()],
        ];
        for lines in &corpus {
            for width in 8u16..=60 {
                for line_numbers in [false, true] {
                    for tab_len in [0u8, 4] {
                        let rows = editor_visual_rows(
                            lines,
                            WrapMode::WordOrGlyph,
                            width,
                            line_numbers,
                            tab_len,
                        );
                        assert!(!rows.is_empty(), "width={width}, ln={line_numbers}");
                        let backend = TestBackend::new(width, rows.len() as u16);
                        let mut terminal = Terminal::new(backend).expect("terminal");
                        let mut textarea = TextArea::from(lines.clone());
                        textarea.set_wrap_mode(WrapMode::WordOrGlyph);
                        textarea.set_tab_length(tab_len);
                        if line_numbers {
                            textarea.set_line_number_style(Style::default());
                        }
                        terminal
                            .draw(|frame| frame.render_widget(&textarea, frame.area()))
                            .expect("render");

                        let digits = lines.len().max(1).to_string().len() as u16;
                        let gutter: u16 = if line_numbers { digits + 2 } else { 0 };
                        for (y, row) in rows.iter().enumerate() {
                            let source = &lines[row.source_line];
                            let fragment: String = source
                                .chars()
                                .skip(row.start_char)
                                .take(row.end_char - row.start_char)
                                .collect();
                            let actual: String = (gutter..width)
                                .filter_map(|x| {
                                    terminal
                                        .backend()
                                        .buffer()
                                        .cell((x, y as u16))
                                        .map(|cell| cell.symbol())
                                })
                                .collect();
                            assert_eq!(
                                actual.trim_end(),
                                display_fragment_cells(&fragment, tab_len).trim_end(),
                                "width={width}, ln={line_numbers}, tab={tab_len}, visual row={y}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn wrapped_highlight_stays_glued_across_scroll_and_resize() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp = tempfile::tempdir().expect("tempdir");
        let mut lines: Vec<String> = vec![
            "# Big Heading One That Is Deliberately Long Enough To Wrap At Eighty Columns Total"
                .into(),
            "some **bold text** and `inline code` plus [a link](https://example.com/xyz)".into(),
            "**an entire paragraph wrapped in bold markers that keeps going and going and going further still**"
                .into(),
            "```rust".into(),
            "fn main() { println!(\"hello wrapped world\"); }".into(),
            "```".into(),
        ];
        for i in 0..40 {
            lines.push(format!(
                "- item {i:02} with **bold** tail and more words to wrap"
            ));
        }

        let mut app = crate::app::App::new(storage(temp.path())).expect("app");
        app.editor.body = EditorDocument::from_lines(lines.clone());
        app.editor.body.set_wrap_mode(WrapMode::WordOrGlyph);
        app.editor.show_line_numbers = true;

        fn check(app: &crate::app::App, terminal: &Terminal<TestBackend>, area: Rect) {
            let buf = terminal.backend().buffer();
            let inner = app.editor.body.inner_rect(area);
            let top = app.editor.body_viewport_row as usize;
            let rows = &app.editor.visual_row_cache.rows;
            let lines_ref = app.editor.body.lines();
            for i in 0..inner.height as usize {
                let Some(row) = rows.get(top + i) else {
                    continue;
                };
                let source = &lines_ref[row.source_line];
                let frag: String = source
                    .chars()
                    .skip(row.start_char)
                    .take(row.end_char - row.start_char)
                    .collect();
                // Expected: fragment with line-number gutter blanked and tabs expanded.
                let expected: String = {
                    let total = lines_ref.len();
                    let digits = total.to_string().len();
                    let n = row.source_line + 1;
                    let mut out = if row.start_char == 0 {
                        let lead = digits - n.to_string().len() + 1;
                        format!("{}{} ", " ".repeat(lead), n)
                    } else {
                        " ".repeat(digits + 2)
                    };
                    let mut width = 0usize;
                    for ch in frag.chars() {
                        if ch == '\t' {
                            let tab = 4usize;
                            let padw = tab - (width % tab);
                            width += padw;
                            out.push_str(&" ".repeat(padw));
                        } else {
                            width += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                            out.push(ch);
                        }
                    }
                    out
                };
                let actual: String = (inner.x..inner.right())
                    .map(|x| {
                        buf.cell((x, inner.y + i as u16))
                            .map(|c| c.symbol())
                            .unwrap_or("")
                    })
                    .collect();
                assert_eq!(
                    actual.trim_end(),
                    expected.trim_end(),
                    "viewport_row={top}, screen row={i}: buffer shows wrong visual row",
                );
            }
        }

        fn render(app: &mut crate::app::App, terminal: &mut Terminal<TestBackend>, area: Rect) {
            terminal
                .draw(|frame| {
                    crate::ui::edit_view::render_editor_widget(
                        frame,
                        app,
                        crate::editor::EditFocus::Body,
                        area,
                        None,
                        None,
                    );
                    overlay_markdown_highlight(frame, app, area);
                })
                .expect("render");
            check(app, terminal, area);
        }

        // Full width (no preview pane)
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
        render(&mut app, &mut terminal, Rect::new(0, 0, 80, 24));
        // Scroll down like a mouse wheel, then partially back up
        app.scroll_editor(10, 0);
        render(&mut app, &mut terminal, Rect::new(0, 0, 80, 24));
        app.scroll_editor(-4, 0);
        render(&mut app, &mut terminal, Rect::new(0, 0, 80, 24));
        // Preview pane on: narrower editor
        let mut narrow = Terminal::new(TestBackend::new(40, 24)).expect("terminal");
        render(&mut app, &mut narrow, Rect::new(0, 0, 40, 24));
        // Preview pane off again: back to full width
        render(&mut app, &mut terminal, Rect::new(0, 0, 80, 24));
    }
    #[test]
    fn edit_markdown_highlight_tracks_soft_wrapped_source_offsets() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp = tempfile::tempdir().expect("tempdir");
        let lines = vec![
            "**bold phrase wraps over narrow visual rows**".into(),
            "[link label wraps over visual rows](https://example.com)".into(),
            "```rust".into(),
            "\tfn wide_界() { println!(\"wrapped\"); }".into(),
            "```".into(),
        ];
        for line_numbers in [false, true] {
            let mut app = crate::app::App::new(storage(temp.path())).expect("app");
            app.editor.body = EditorDocument::from_lines(lines.clone());
            app.editor.body.set_wrap_mode(WrapMode::WordOrGlyph);
            app.editor.show_line_numbers = line_numbers;
            let backend = TestBackend::new(16, 16);
            let mut terminal = Terminal::new(backend).expect("terminal");
            terminal
                .draw(|frame| {
                    render_editor_document_with_theme(
                        frame,
                        &mut app.editor.body,
                        frame.area(),
                        &app.app_theme,
                        true,
                        line_numbers,
                        Block::default(),
                        app.app_theme.bg_style(),
                    );
                    overlay_markdown_highlight(frame, &mut app, frame.area());
                })
                .expect("render");

            let gutter: u16 = if line_numbers { 3 } else { 0 };
            for (visual_index, row) in app.editor.visual_row_cache.rows.iter().enumerate() {
                if visual_index >= 16 {
                    break;
                }
                let source = &lines[row.source_line];
                let mut highlighter = crate::markdown::SourceHighlighter::new(
                    &app.app_theme,
                    app.config.editor.ghost_syntax,
                    app.config.editor.extended_markdown_features,
                );
                highlighter.rescan(&lines);
                let styles = highlighter.highlight_line(source, row.source_line, false);
                let mut x = gutter;
                let mut display_width = 0;
                for (offset, ch) in source
                    .chars()
                    .skip(row.start_char)
                    .take(row.end_char - row.start_char)
                    .enumerate()
                {
                    let style = styles[row.start_char + offset];
                    let width = if ch == '\t' {
                        4 - (display_width % 4)
                    } else {
                        ch.width().unwrap_or(0)
                    };
                    if let Some(expected) = style.fg {
                        let painted_cells = if ch == '\t' { width } else { width.min(1) };
                        for cell_x in x..x.saturating_add(painted_cells as u16).min(16) {
                            let cell = terminal
                                .backend()
                                .buffer()
                                .cell((cell_x, visual_index as u16))
                                .expect("cell");
                            if !cell.symbol().is_empty()
                                && !cell.modifier.contains(Modifier::REVERSED)
                                && cell.bg == app.app_theme.bg.unwrap_or(Color::Reset)
                            {
                                assert_eq!(
                                    cell.fg, expected,
                                    "line_numbers={line_numbers}, source_line={}, chars={}..{}, ch={ch:?}, visual row={visual_index}, x={cell_x}",
                                    row.source_line, row.start_char, row.end_char,
                                );
                            }
                        }
                    }
                    x = x.saturating_add(width as u16);
                    display_width += width;
                }
            }
        }
    }

    #[test]
    fn visible_highlight_cache_handles_fences_modes_settings_and_scroll() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let root = tempfile::tempdir().expect("tempdir");
        let mut app = App::new(storage(root.path())).expect("app");
        let mut lines: Vec<_> = (0..2_000)
            .map(|row| format!("**界 e\u{301}**\tline {row}"))
            .collect();
        lines[0] = "```rust".into();
        lines[100] = "```".into();
        app.editor.body = EditorDocument::from_lines(lines);
        let mut terminal = Terminal::new(TestBackend::new(22, 8)).expect("terminal");
        fn draw(app: &mut App, terminal: &mut Terminal<TestBackend>) -> ratatui::buffer::Buffer {
            terminal
                .draw(|frame| {
                    edit_view::render_editor_widget(
                        frame,
                        app,
                        EditFocus::Body,
                        frame.area(),
                        Some(Block::default()),
                        None,
                    );
                    overlay_markdown_highlight(frame, app, frame.area());
                })
                .expect("draw");
            terminal.backend().buffer().clone()
        }
        for wrap in [WrapMode::None, WrapMode::WordOrGlyph] {
            app.editor.body.set_wrap_mode(wrap);
            for numbers in [false, true] {
                app.editor.show_line_numbers = numbers;
                app.editor
                    .body
                    .move_cursor(ratatui_textarea::CursorMove::Jump(40, 0));
                draw(&mut app, &mut terminal);
                assert!(
                    app.editor.md_highlight_memo.len() <= 24,
                    "only visible lines get styles"
                );
                for step in 0..6 {
                    match step {
                        0 => {
                            app.editor
                                .body
                                .move_cursor(ratatui_textarea::CursorMove::Top);
                            app.editor.body.insert_str(" ");
                        }
                        1 => {
                            app.config.editor.ghost_syntax = !app.config.editor.ghost_syntax;
                        }
                        2 => {
                            app.config.editor.extended_markdown_features =
                                !app.config.editor.extended_markdown_features;
                        }
                        3 => {
                            app.app_theme.muted = Color::Red;
                        }
                        4 => {
                            app.editor.editing_id = Some("todo.txt".into());
                        }
                        _ => {
                            app.editor.editing_id = Some("note.md".into());
                            app.editor.body.undo();
                        }
                    }
                    app.editor
                        .body
                        .move_cursor(ratatui_textarea::CursorMove::Jump(45, 12));
                    let cached = draw(&mut app, &mut terminal);
                    app.editor.source_highlighter = None;
                    let fresh = draw(&mut app, &mut terminal);
                    assert_eq!(
                        cached, fresh,
                        "wrap={wrap:?}, numbers={numbers}, step={step}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_zen_focus_range() {
        let lines: Vec<String> = vec![
            "p1 line 1".into(),
            "p1 line 2".into(),
            "".into(),
            "p2 line 1".into(),
            "".into(),
            "".into(),
            "p3 line 1".into(),
            "p3 line 2".into(),
            "p3 line 3".into(),
        ];

        // Focus Unit: Line
        assert_eq!(
            zen_focus_range(&lines, 7, crate::config::FocusUnit::Line, 2),
            5..8
        );
        // Clamps at 0
        assert_eq!(
            zen_focus_range(&lines, 1, crate::config::FocusUnit::Line, 5),
            0..2
        );

        // Focus Unit: Paragraph, context = 1
        // Cursor in p3 (row 7) -> expects p2 (row 3) and p3 (rows 6..=8) -> range 3..9
        assert_eq!(
            zen_focus_range(&lines, 7, crate::config::FocusUnit::Paragraph, 1),
            3..9
        );

        // Focus Unit: Paragraph, context = 2
        // Cursor in p3 -> expects p1, p2, p3 -> range 0..9
        assert_eq!(
            zen_focus_range(&lines, 7, crate::config::FocusUnit::Paragraph, 2),
            0..9
        );

        // Cursor on blank line 4, context = 1
        // current block is empty (4..=4). 1 block above is p2 (row 3). -> range 3..5
        assert_eq!(
            zen_focus_range(&lines, 4, crate::config::FocusUnit::Paragraph, 1),
            3..5
        );

        // Context 0 -> only current paragraph
        assert_eq!(
            zen_focus_range(&lines, 7, crate::config::FocusUnit::Paragraph, 0),
            6..9
        );
    }

    #[test]
    fn zen_focus_dimming_render() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = crate::app::App::new(storage(temp.path())).expect("app");

        app.zen_mode = true;
        app.config.editor.zen_focus_dimming = true;
        app.config.editor.zen_focus_unit = crate::config::FocusUnit::Paragraph;
        app.config.editor.zen_focus_context = 0; // Only current paragraph

        let lines = vec![
            "paragraph 1".into(),
            "".into(),
            "paragraph 2".into(),
            "".into(),
            "paragraph 3".into(),
        ];
        app.editor.body = EditorDocument::from_lines(lines);
        // Cursor on paragraph 2
        app.editor.body.input(ratatui_textarea::Input {
            key: ratatui_textarea::Key::Down,
            ctrl: false,
            alt: false,
            shift: false,
        });
        app.editor.body.input(ratatui_textarea::Input {
            key: ratatui_textarea::Key::Down,
            ctrl: false,
            alt: false,
            shift: false,
        });

        let backend = ratatui::backend::TestBackend::new(20, 10);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");

        terminal
            .draw(|frame| {
                let area = ratatui::layout::Rect::new(0, 0, 20, 10);
                crate::ui::edit_view::render_editor_widget(
                    frame,
                    &mut app,
                    crate::app::EditFocus::Body,
                    area,
                    None,
                    None,
                );
                overlay_zen_focus(frame, &mut app, area);
            })
            .expect("render");

        let buf = terminal.backend().buffer();
        // paragraph 1 (row 1 because of top padding) should be dimmed
        assert!(
            buf.cell((0, 1))
                .expect("palette is some so p_tabs is some")
                .modifier
                .contains(ratatui::style::Modifier::DIM)
        );
        // paragraph 2 (row 3) should NOT be dimmed
        assert!(
            !buf.cell((0, 3))
                .expect("palette is some so p_tabs is some")
                .modifier
                .contains(ratatui::style::Modifier::DIM)
        );
        // paragraph 3 (row 5) should be dimmed
        assert!(
            buf.cell((0, 5))
                .expect("palette is some so p_tabs is some")
                .modifier
                .contains(ratatui::style::Modifier::DIM)
        );
    }

    #[test]
    fn zen_hide_line_numbers_render() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = crate::app::App::new(storage(temp.path())).expect("app");

        app.zen_mode = true;
        app.editor.show_line_numbers = true;
        app.config.editor.zen_hide_line_numbers = true;

        let lines = vec!["hello world".into()];
        app.editor.body = EditorDocument::from_lines(lines);

        let backend = ratatui::backend::TestBackend::new(20, 5);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");

        terminal
            .draw(|frame| {
                let area = ratatui::layout::Rect::new(0, 0, 20, 5);
                crate::ui::edit_view::render_editor_widget(
                    frame,
                    &mut app,
                    crate::app::EditFocus::Body,
                    area,
                    None,
                    None,
                );
            })
            .expect("render");

        let buf = terminal.backend().buffer();
        // Line numbers are hidden, so the first text cell (row 1 due to padding) should be 'h'.
        assert_eq!(
            buf.cell((0, 1))
                .expect("palette is some so p_tabs is some")
                .symbol(),
            "h"
        );
    }
}
