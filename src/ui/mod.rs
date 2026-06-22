use std::path::Path;
use std::process::Command;
use anyhow::{Context, Result};
use ratatui::{prelude::*, widgets::*};
use ratatui_textarea::TextArea;

use crate::app::{App, EditFocus, ViewMode};
use crate::app_theme::AppThemeColors;

mod list_view;
mod edit_view;
mod popups;
mod title_bar;
mod help;

pub(crate) use list_view::{draw_list_view, get_preview_info, list_view_layout};
pub use edit_view::draw_edit_view;
pub use popups::*;
pub use title_bar::*;
pub use help::*;

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
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                draw_view_title_bar(frame, outer[0], "Graph", &app.app_theme, None);
                graf.overlay_render(frame, outer[1], &app.app_theme, &app.config);
            }
        }
        ViewMode::Draw => {
            if let Some(draw) = &mut app.draw_state {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                draw_view_title_bar(frame, outer[0], "Draw", &app.app_theme, None);
                draw.overlay_render(frame, outer[1], &app.app_theme, &app.config);
            }
        }
        ViewMode::Canvas => {
            if let Some(canvas) = &mut app.canvas_state {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                draw_view_title_bar(frame, outer[0], "Canvas", &app.app_theme, None);
                canvas.overlay_render(frame, outer[1], &app.app_theme, &app.config);
            }
        }
        ViewMode::Backup => {
            if let Some(backup) = &mut app.backup_state {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                // Backup uses a custom header render instead of the standard title bar
                crate::backup::render::draw_header(frame, outer[0], backup);
                backup.overlay_render(frame, outer[1], &app.app_theme, &app.config);
            }
        }
        ViewMode::ContentTree => {
            if let Some(tree) = &mut app.content_tree_state {
                let outer = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(frame.area());
                let title = format!("CONTENT TREE — {}", tree.note_title);
                draw_view_title_bar(frame, outer[0], &title, &app.app_theme, None);
                tree.overlay_render(frame, outer[1], &app.app_theme, &app.config);
            }
        }
    }

    // Global popups — rendered on top of the active view
    // Template popup
    if let Some(popup) = &app.popups.template {
        draw_template_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Folder popup
    if let Some(popup) = &mut app.popups.folder {
        let title = match popup.mode {
            crate::popups::FolderPopupMode::Create { .. } => "NEW FOLDER",
            crate::popups::FolderPopupMode::Rename { .. } => "RENAME FOLDER",
        };
        let content = draw_popup_frame(
            frame,
            frame.area(),
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

    // Tag popup
    if let Some(popup) = &mut app.popups.tag {
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

    // Folder picker popup
    if let Some(picker) = &mut app.popups.folder_picker {
        let title = match picker.mode {
            crate::popups::FolderPickerMode::CopyNote { .. } => "COPY",
            _ => "MOVE",
        };
        let content = draw_popup_frame(
            frame,
            frame.area(),
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

    // Command palette
    if let Some(palette) = &mut app.command_palette {
        let area = frame.area();
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

    // Note rename popup
    if let Some(popup) = &mut app.popups.note_rename {
        let content = draw_popup_frame(
            frame,
            frame.area(),
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

    // Goals popup
    if let Some(popup) = &mut app.popups.goals {
        let (title, sub) = match popup.mode {
            crate::popups::GoalsPopupMode::WordGoal => {
                ("DAILY WORD GOAL", "Enter word count · Esc cancel")
            }
            crate::popups::GoalsPopupMode::NoteGoal => {
                ("DAILY NOTE GOAL", "Enter note count · Esc cancel")
            }
        };
        let content = draw_popup_frame(frame, frame.area(), title, PopupSize::Prompt, sub, &app.app_theme);

        popup.input.set_block(
            Block::default()
                .style(app.app_theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.app_theme.heading)),
        );
        frame.render_widget(&popup.input, content);
    }

    // Create note popup
    if let Some((popup, format)) = &mut app.popups.create_note {
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
            "Enter create · Esc cancel",
            &app.app_theme,
        );
        popup.input.set_block(popup_block("", &app.app_theme));
        frame.render_widget(&popup.input, content);
    }

    // Import popup
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
            frame.area(),
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

    // Search popup
    if let Some(popup) = &mut app.popups.search {
        let area = frame.area();
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

    // Trash view popup
    if let Some(trash) = &app.popups.trash_view {
        let area = frame.area();
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

        let mut state = ListState::default();
        state.select(Some(trash.selected));

        frame.render_stateful_widget(list, content, &mut state);
    }

    // Confirm popup
    if let Some(popup) = &app.popups.confirm {
        draw_confirm_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Theme popup
    if let Some(popup) = &app.popups.theme {
        draw_theme_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Sort popup
    if let Some(popup) = &app.popups.sort {
        draw_sort_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Create format popup
    if let Some(popup) = &app.popups.create_format {
        draw_create_format_popup(frame, popup, frame.area(), &app.app_theme);
    }

    // Context menu (from edit view)
    if let Some(menu) = &app.popups.context_menu {
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

        let mut state = ListState::default();
        state.select(Some(menu.selected));

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
