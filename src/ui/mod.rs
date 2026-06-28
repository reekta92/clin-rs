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
mod list_view;
mod popups;
mod title_bar;

pub use edit_view::draw_edit_view;
pub use help::*;
pub(crate) use list_view::{draw_list_view, get_preview_info, list_view_layout};
pub use popups::*;
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
    if let Some(_bg) = app.app_theme.bg {
        let block = Block::default().style(app.app_theme.bg_style());
        frame.render_widget(block, frame.area());
    }

    match app.mode {
        ViewMode::List => draw_list_view(frame, app),
        ViewMode::Edit => draw_edit_view(frame, app, focus),
        ViewMode::Help => draw_help_view(frame, app),
        ViewMode::Graph => {
            if let Some(graf) = &mut app.graph_state {
                graf.overlay_render(
                    frame,
                    frame.area(),
                    &app.app_theme,
                    &app.config,
                    Some(app.status.as_ref()),
                );
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
                let tabs: Vec<(&str, Option<&str>)> =
                    tabs_arr.iter().map(|&(l, g)| (l, Some(g))).collect();
                let active = crate::draw::render::draw_tool_tab_index(draw.active_tool);
                let spans = build_tab_spans(&tabs, active, &app.app_theme, false, icon_mode);
                draw_view_title_bar_with_tabs(
                    frame,
                    outer[0],
                    "Draw",
                    spans,
                    &app.app_theme,
                    Some(app.status.as_ref()),
                    None,
                );
                draw.overlay_render(frame, outer[1], &app.app_theme, &app.config, None);
            }
        }
        ViewMode::Canvas => {
            if let Some(canvas) = &mut app.canvas_state {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                draw_view_title_bar(
                    frame,
                    outer[0],
                    "Canvas",
                    &app.app_theme,
                    None,
                    Some(app.status.as_ref()),
                    None,
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
                    backup,
                    app.config.ui.icon_mode,
                );
                backup.overlay_render(frame, outer[1], &app.app_theme, &app.config, None);
            }
        }
        ViewMode::ContentTree => {
            if let Some(tree) = &mut app.content_tree_state {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                let title = format!("CONTENT TREE — {}", tree.note_title);
                draw_view_title_bar(
                    frame,
                    outer[0],
                    &title,
                    &app.app_theme,
                    None,
                    Some(app.status.as_ref()),
                    None,
                );
                tree.overlay_render(frame, outer[1], &app.app_theme, &app.config, None);
            }
        }
    }

    // Global popups — rendered on top of the active view
    // Template popup
    if let Some(crate::popups::ActivePopup::Template(popup)) = &app.popups.active {
        draw_template_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Folder popup
    if let Some(crate::popups::ActivePopup::Folder(popup)) = &mut app.popups.active {
        let title = match popup.mode {
            crate::popups::FolderPopupMode::Create { .. } => "NEW FOLDER",
            crate::popups::FolderPopupMode::Rename { .. } => "RENAME FOLDER",
        };
        let hint_line = popup_hint_line(&app.app_theme, "Enter confirm · Esc cancel");
        let content = draw_popup_frame(
            frame,
            frame.area(),
            title,
            PopupSize::Prompt,
            &hint_line,
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
        let hint_line = popup_hint_line(
            &app.app_theme,
            "Ctrl+S batch assign · Tab accept · Enter save · d delete from all · Esc cancel",
        );
        let content = draw_popup_frame(
            frame,
            frame.area(),
            "TAGS",
            PopupSize::Large,
            &hint_line,
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

        let mut tags_state = list_state_selected(
            (popup.focus == crate::popups::TagPopupFocus::AllTagsList
                && !popup.all_tags.is_empty())
            .then_some(popup.all_tags_selected),
        );
        frame.render_stateful_widget(tags_list, chunks[1], &mut tags_state);
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
        let hint_line = popup_hint_line(&app.app_theme, "Tab switch  Enter confirm  Esc cancel");
        let content = draw_popup_frame(
            frame,
            frame.area(),
            title,
            PopupSize::Large,
            &hint_line,
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

        let mut state = list_state_selected(
            (picker.focus == crate::app::FolderPickerFocus::Results
                && !picker.filtered_folders.is_empty())
            .then_some(picker.selected),
        );

        frame.render_stateful_widget(list, chunks[1], &mut state);
    }

    // Command palette
    if let Some(palette) = &mut app.command_palette {
        let area = frame.area();
        let hint_line = popup_hint_line(
            &app.app_theme,
            "Tab category · Enter run · ↑/↓ select · Esc close",
        );
        let content = draw_popup_frame(
            frame,
            area,
            "COMMANDS",
            PopupSize::Large,
            &hint_line,
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
        let tab_spans = build_tab_spans(
            &tabs,
            palette.active_tab,
            &app.app_theme,
            app.config.ui.tab_icons_only,
            app.config.ui.icon_mode,
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

    // Note rename popup
    if let Some(crate::popups::ActivePopup::NoteRename(popup)) = &mut app.popups.active {
        let hint_line = popup_hint_line(&app.app_theme, "Enter rename · Esc cancel");
        let content = draw_popup_frame(
            frame,
            frame.area(),
            "RENAME",
            PopupSize::Prompt,
            &hint_line,
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
        let (title, sub) = match popup.mode {
            crate::popups::GoalsPopupMode::WordGoal => {
                ("DAILY WORD GOAL", "Enter word count · Esc cancel")
            }
            crate::popups::GoalsPopupMode::NoteGoal => {
                ("DAILY NOTE GOAL", "Enter note count · Esc cancel")
            }
        };
        let hint_line = popup_hint_line(&app.app_theme, sub);
        let content = draw_popup_frame(
            frame,
            frame.area(),
            title,
            PopupSize::Prompt,
            &hint_line,
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
        let hint_line = popup_hint_line(&app.app_theme, "Enter create · Esc cancel");
        let content = draw_popup_frame(
            frame,
            frame.area(),
            title,
            PopupSize::Prompt,
            &hint_line,
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
        let hint_line = popup_hint_line(&app.app_theme, "Enter import · Esc cancel");
        let content = draw_popup_frame(
            frame,
            frame.area(),
            title,
            PopupSize::Large,
            &hint_line,
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
        let hint_line = popup_hint_line(
            &app.app_theme,
            "Tab switch · Enter open · Esc cancel · f:folder p:pinned t:tag g:text · \\e\\ escapes filters",
        );
        let content = draw_popup_frame(
            frame,
            area,
            "SEARCH",
            PopupSize::Large,
            &hint_line,
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
            let items: Vec<ListItem> = visible
                .iter()
                .map(|(_, t)| {
                    ListItem::new(crate::ui::styled_result_line(
                        t,
                        &app.app_theme,
                        app.config.ui.icon_mode,
                    ))
                })
                .collect();
            (items, "")
        } else if has_title {
            let items: Vec<ListItem> = popup
                .title_results
                .iter()
                .map(|entry| {
                    ListItem::new(crate::ui::styled_result_line(
                        entry,
                        &app.app_theme,
                        app.config.ui.icon_mode,
                    ))
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
        let hint_line = popup_hint_line(&app.app_theme, "r restore · d delete · E empty · q close");
        let content = draw_popup_frame(
            frame,
            area,
            "TRASH",
            PopupSize::Large,
            &hint_line,
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

        let mut state = list_state_selected(Some(trash.selected));

        frame.render_stateful_widget(list, content, &mut state);
    }

    // Confirm popup
    if let Some(popup) = &app.popups.confirm {
        draw_confirm_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Theme popup
    if let Some(crate::popups::ActivePopup::Theme(popup)) = &app.popups.active {
        draw_theme_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Sort popup
    // Icon mode popup
    if let Some(crate::popups::ActivePopup::IconMode(popup)) = &app.popups.active {
        draw_icon_mode_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Hint bar style popup
    if let Some(crate::popups::ActivePopup::HintBarStyle(popup)) = &app.popups.active {
        draw_hint_bar_style_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Keybind preset popup
    if let Some(crate::popups::ActivePopup::KeybindPreset(popup)) = &app.popups.active {
        draw_keybind_preset_popup(frame, popup, frame.area(), &app.app_theme);
    }

    if let Some(crate::popups::ActivePopup::Sort(popup)) = &app.popups.active {
        draw_sort_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Create format popup
    if let Some(crate::popups::ActivePopup::CreateFormat(popup)) = &app.popups.active {
        draw_create_format_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Context menu (from edit view)
    if let Some(crate::popups::ActivePopup::ContextMenu(menu)) = &app.popups.active {
        let labels = [" Copy ", " Cut ", " Paste ", " Select All "];
        let items: Vec<ListItem> = labels.iter().map(|l| ListItem::new(*l)).collect();
        let menu_width = labels.iter().map(|l| l.len() as u16).max().unwrap_or(0);
        let menu_height = labels.len() as u16;

        let list = List::new(items)
            .block(
                Block::default()
                    .style(app.app_theme.preview_bg_style())
                    .borders(Borders::NONE),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let x = menu.x.min(frame.area().width.saturating_sub(menu_width));
        let y = menu.y.min(frame.area().height.saturating_sub(menu_height));
        let menu_area = Rect::new(x, y, menu_width, menu_height);

        let mut state = list_state_selected(Some(menu.selected));

        frame.render_widget(Clear, menu_area);
        frame.render_stateful_widget(list, menu_area, &mut state);
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

pub fn pick_file(filter_name: &str, filter_ext: &str) -> Result<Option<String>> {
    if cfg!(target_os = "linux") {
        if which::which("zenity").is_ok() {
            let output = Command::new("zenity")
                .arg("--file-selection")
                .arg(format!("--file-filter={filter_name} | *{filter_ext}"))
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
                .arg(format!("*{filter_ext}"))
                .output()?;
            if output.status.success() {
                return Ok(Some(
                    String::from_utf8_lossy(&output.stdout).trim().to_string(),
                ));
            }
        }
    } else if cfg!(target_os = "macos") {
        let posix_script = format!(
            "POSIX path of (choose file with prompt \"Select a {} file\" of type {{\"{}\"}})",
            filter_name,
            filter_ext.trim_start_matches('.')
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
        let ps_script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; $f.Filter = '{filter_name} (*{filter_ext})|*{filter_ext}'; $f.ShowDialog() | Out-Null; $f.FileName"
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
