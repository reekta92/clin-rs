use ratatui::{prelude::*, widgets::*};
use ratatui_textarea::TextArea;
use std::borrow::Cow;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::PopupSize;
use crate::app::{ConfirmPopup, TemplatePopup, ThemePopup};
use crate::app_theme::AppThemeColors;

pub fn draw_template_popup(
    frame: &mut Frame,
    popup: &mut TemplatePopup,
    area: Rect,
    theme: &AppThemeColors,
    mouse_pos: Option<(u16, u16)>,
) {
    let content = draw_popup_frame(
        frame,
        area,
        "TEMPLATES",
        PopupSize::Large,
        PopupHints::Keybinds(&[
            ("Tab".to_string(), "switch"),
            ("Enter".to_string(), "use template"),
            ("n".to_string(), "create"),
            ("d".to_string(), "delete"),
            ("Space".to_string(), "edit"),
            ("?".to_string(), "help"),
            ("Esc".to_string(), "cancel"),
        ]),
        theme,
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(content);

    let mut input = popup.input.clone();
    input.set_style(theme.bg_style());
    input.set_block(
        Block::default()
            .style(theme.bg_style())
            .borders(Borders::ALL)
            .border_style(
                if popup.focus == crate::popups::TemplatePopupFocus::Search {
                    Style::default().fg(theme.heading)
                } else {
                    Style::default().fg(theme.muted)
                },
            )
            .title(""),
    );
    frame.render_widget(&input, chunks[0]);

    let items: Vec<ListItem> = if popup.filtered_templates.is_empty() {
        empty_list_item(theme, "(no matching templates)")
    } else {
        popup
            .filtered_templates
            .iter()
            .map(|t| {
                ListItem::new(Line::from(vec![
                    Span::styled(&t.name, Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("  ({})", t.filename),
                        Style::default().fg(theme.muted),
                    ),
                ]))
            })
            .collect()
    };

    let results_border = if popup.focus == crate::popups::TemplatePopupFocus::Results {
        Style::default().fg(theme.heading)
    } else {
        Style::default().fg(theme.muted)
    };
    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(results_border),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("  ");

    let mut state = list_state_selected(
        if popup.focus == crate::popups::TemplatePopupFocus::Results
            && !popup.filtered_templates.is_empty()
        {
            Some(popup.selected)
        } else {
            None
        },
        popup.scroll_offset,
    );

    frame.render_stateful_widget(list, chunks[1], &mut state);
    popup.scroll_offset = state.offset();
    paint_list_hover(
        frame,
        Rect {
            x: chunks[1].x + 1,
            y: chunks[1].y + 1,
            width: chunks[1].width.saturating_sub(2),
            height: chunks[1].height.saturating_sub(2),
        },
        &state,
        popup.filtered_templates.len(),
        mouse_pos,
        theme.hover_style(),
    );
    let list_inner = Rect {
        x: chunks[1].x + 1,
        y: chunks[1].y + 1,
        width: chunks[1].width.saturating_sub(2),
        height: chunks[1].height.saturating_sub(2),
    };
    popup.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
        track: crate::ui::scrollbar::track_rect(list_inner),
        content_len: popup.filtered_templates.len(),
        viewport_len: list_inner.height as usize,
    });
    crate::ui::scrollbar::draw_scrollbar(
        frame,
        list_inner,
        popup.filtered_templates.len(),
        list_inner.height as usize,
        popup.selected,
        popup.filtered_templates.len().saturating_sub(1),
        theme,
    );
}

pub fn draw_info_popup(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    popup: &crate::popups::InfoPopup,
    theme: &crate::app_theme::AppThemeColors,
) {
    let inner = crate::ui::draw_popup_frame(
        frame,
        area,
        &popup.title,
        crate::ui::PopupSize::Medium,
        PopupHints::Keybinds(&[("Enter/Esc".to_string(), "close")]),
        theme,
    );
    // Inner border, background, and padding matching other popup styles
    let block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .padding(Padding::new(1, 1, 0, 0));
    let content_area = block.inner(inner);
    frame.render_widget(block, inner);

    // Available width for text word-wrapping (inside the padded content area)
    let width = content_area.width.saturating_sub(2).max(1) as usize;
    // Build vertical layout constraints matching each InfoItem
    let mut constraints: Vec<Constraint> = Vec::with_capacity(popup.items.len() + 1);
    for item in &popup.items {
        match item {
            crate::popups::InfoItem::Metrics(pairs) => {
                constraints.push(Constraint::Length(pairs.len() as u16));
            }
            crate::popups::InfoItem::Spacer => {
                constraints.push(Constraint::Length(1));
            }
            crate::popups::InfoItem::Text { heading: _, body } => {
                let body_lines: u16 = body
                    .lines()
                    .map(|line| {
                        let line_len = line.chars().count();
                        if line_len == 0 {
                            1u16
                        } else {
                            (line_len / width) as u16 + 1
                        }
                    })
                    .sum();
                constraints.push(Constraint::Length(1 + body_lines));
            }
        }
    }
    constraints.push(Constraint::Min(0)); // consume remaining space

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(content_area);

    for (idx, item) in popup.items.iter().enumerate() {
        let item_area = chunks[idx];
        match item {
            crate::popups::InfoItem::Metrics(pairs) => {
                let max_key_len = pairs
                    .iter()
                    .map(|(k, _)| k.chars().count())
                    .max()
                    .unwrap_or(0) as u16;

                let rows: Vec<Row> = pairs
                    .iter()
                    .map(|(key, value)| {
                        Row::new(vec![
                            Cell::from(key.as_str()).style(Style::default().fg(theme.accent)),
                            Cell::from(value.as_str()).style(Style::default().fg(theme.fg)),
                        ])
                    })
                    .collect();

                let table = Table::new(
                    rows,
                    [Constraint::Length(max_key_len + 2), Constraint::Min(0)],
                );
                frame.render_widget(table, item_area);
            }
            crate::popups::InfoItem::Spacer => {}
            crate::popups::InfoItem::Text { heading, body } => {
                let text_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(0)])
                    .split(item_area);

                let heading_para =
                    Paragraph::new(heading.as_str()).style(Style::default().fg(theme.accent));
                frame.render_widget(heading_para, text_chunks[0]);

                let body_para = Paragraph::new(body.as_str())
                    .style(Style::default().fg(theme.fg))
                    .wrap(Wrap { trim: true });
                frame.render_widget(body_para, text_chunks[1]);
            }
        }
    }
}

pub fn draw_theme_popup(
    frame: &mut Frame,
    popup: &mut ThemePopup,
    area: Rect,
    theme: &AppThemeColors,
    keybinds: &crate::keybinds::Keybinds,
    mouse_pos: Option<(u16, u16)>,
) {
    let content = draw_popup_frame(
        frame,
        area,
        "THEMES",
        PopupSize::Medium,
        PopupHints::Keybinds(&[
            (
                keybinds.display_list(crate::keybinds::ListAction::CycleFocus),
                "navigate",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Confirm),
                "select",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Cancel),
                "close",
            ),
        ]),
        theme,
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(content);

    let items: Vec<ListItem> = popup
        .themes
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut spans = vec![Span::raw(t)];
            if popup.is_custom.get(i).copied().unwrap_or(false) {
                spans.push(Span::styled(" [custom]", Style::default().fg(theme.muted)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list_style = if popup.focus == crate::app::ThemePopupFocus::ThemeList {
        Style::default().fg(theme.heading)
    } else {
        Style::default().fg(theme.muted)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(list_style),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    let state = render_list_with_selection(
        frame,
        list,
        chunks[0],
        Some(popup.selected),
        popup.scroll_offset,
    );
    popup.scroll_offset = state.offset();
    paint_list_hover(
        frame,
        Rect {
            x: chunks[0].x + 1,
            y: chunks[0].y + 1,
            width: chunks[0].width.saturating_sub(2),
            height: chunks[0].height.saturating_sub(2),
        },
        &state,
        popup.themes.len(),
        mouse_pos,
        theme.hover_style(),
    );
    let theme_inner = Rect {
        x: chunks[0].x + 1,
        y: chunks[0].y + 1,
        width: chunks[0].width.saturating_sub(2),
        height: chunks[0].height.saturating_sub(2),
    };
    popup.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
        track: crate::ui::scrollbar::track_rect(theme_inner),
        content_len: popup.themes.len(),
        viewport_len: theme_inner.height as usize,
    });
    crate::ui::scrollbar::draw_scrollbar(
        frame,
        theme_inner,
        popup.themes.len(),
        theme_inner.height as usize,
        popup.selected,
        popup.themes.len().saturating_sub(1),
        theme,
    );

    let gen_label = if popup.general_is_solid {
        "General Background Color: ON"
    } else {
        "General Background Color: OFF"
    };
    let graph_label = if popup.graph_is_solid {
        "Graph Background Color: ON"
    } else {
        "Graph Background Color: OFF"
    };

    let gen_hovered =
        mouse_pos.is_some_and(|(col, row)| crate::events::contains_cell(chunks[1], col, row));
    let graph_hovered =
        mouse_pos.is_some_and(|(col, row)| crate::events::contains_cell(chunks[2], col, row));

    let gen_style = if popup.general_is_solid {
        Style::default().fg(theme.success)
    } else {
        Style::default().fg(theme.destructive)
    };
    let gen_block_style = if gen_hovered {
        theme.hover_style()
    } else {
        theme.bg_style()
    };
    let gen_block = Block::default()
        .style(gen_block_style)
        .borders(Borders::ALL)
        .border_style(if popup.focus == crate::app::ThemePopupFocus::GeneralBg {
            Style::default().fg(theme.heading)
        } else {
            Style::default().fg(theme.muted)
        });
    let gen_inner = gen_block.inner(chunks[1]);
    let gen_para = Paragraph::new(Span::styled(gen_label, gen_style))
        .alignment(Alignment::Center)
        .style(theme.bg_style());
    frame.render_widget(gen_block, chunks[1]);
    frame.render_widget(gen_para, gen_inner);

    let graph_style = if popup.graph_is_solid {
        Style::default().fg(theme.success)
    } else {
        Style::default().fg(theme.destructive)
    };
    let graph_block_style = if graph_hovered {
        theme.hover_style()
    } else {
        theme.bg_style()
    };
    let graph_block = Block::default()
        .style(graph_block_style)
        .borders(Borders::ALL)
        .border_style(if popup.focus == crate::app::ThemePopupFocus::GraphBg {
            Style::default().fg(theme.heading)
        } else {
            Style::default().fg(theme.muted)
        });
    let graph_inner = graph_block.inner(chunks[2]);
    let graph_para = Paragraph::new(Span::styled(graph_label, graph_style))
        .alignment(Alignment::Center)
        .style(theme.bg_style());
    frame.render_widget(graph_block, chunks[2]);
    frame.render_widget(graph_para, graph_inner);
}
pub fn draw_sort_popup(
    frame: &mut Frame,
    popup: &crate::popups::SortPopup,
    area: Rect,
    theme: &AppThemeColors,
    keybinds: &crate::keybinds::Keybinds,
    mouse_pos: Option<(u16, u16)>,
) {
    let content_area = draw_popup_frame(
        frame,
        area,
        "SORT BY",
        PopupSize::Medium,
        PopupHints::Keybinds(&[
            (
                keybinds.display_list(crate::keybinds::ListAction::MoveUp),
                "up",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::MoveDown),
                "down",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Confirm),
                "select",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Cancel),
                "cancel",
            ),
        ]),
        theme,
    );

    let options = [
        "Title (A-Z)",
        "Title (Z-A)",
        "Modified (newest)",
        "Modified (oldest)",
    ];
    let items: Vec<ListItem> = options
        .iter()
        .map(|&opt| ListItem::new(Line::from(Span::raw(opt))))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.heading)),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    let state = render_list_with_selection(frame, list, content_area, Some(popup.selected), 0);
    paint_list_hover(
        frame,
        Rect {
            x: content_area.x + 1,
            y: content_area.y + 1,
            width: content_area.width.saturating_sub(2),
            height: content_area.height.saturating_sub(2),
        },
        &state,
        options.len(),
        mouse_pos,
        theme.hover_style(),
    );
}
pub fn draw_icon_mode_popup(
    frame: &mut Frame,
    popup: &crate::popups::IconModePopup,
    area: Rect,
    theme: &AppThemeColors,
    keybinds: &crate::keybinds::Keybinds,
    mouse_pos: Option<(u16, u16)>,
) {
    let content_area = draw_popup_frame(
        frame,
        area,
        "ICON MODE",
        PopupSize::Medium,
        PopupHints::Keybinds(&[
            (
                keybinds.display_list(crate::keybinds::ListAction::MoveUp),
                "up",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::MoveDown),
                "down",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Confirm),
                "select",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Cancel),
                "cancel",
            ),
        ]),
        theme,
    );

    let options = ["Nerd Font", "Unicode", "None"];
    let items: Vec<ListItem> = options
        .iter()
        .map(|&opt| ListItem::new(Line::from(Span::raw(opt))))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.heading)),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    let state = render_list_with_selection(frame, list, content_area, Some(popup.selected), 0);
    paint_list_hover(
        frame,
        Rect {
            x: content_area.x + 1,
            y: content_area.y + 1,
            width: content_area.width.saturating_sub(2),
            height: content_area.height.saturating_sub(2),
        },
        &state,
        options.len(),
        mouse_pos,
        theme.hover_style(),
    );
}
pub fn draw_create_format_popup(
    frame: &mut Frame,
    popup: &crate::popups::CreateFormatPopup,
    area: Rect,
    theme: &AppThemeColors,
    keybinds: &crate::keybinds::Keybinds,
    mouse_pos: Option<(u16, u16)>,
) {
    let content_area = draw_popup_frame(
        frame,
        area,
        "CREATE NEW",
        PopupSize::Medium,
        PopupHints::Keybinds(&[
            (
                keybinds.display_list(crate::keybinds::ListAction::MoveUp),
                "up",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::MoveDown),
                "down",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Confirm),
                "select",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Cancel),
                "cancel",
            ),
        ]),
        theme,
    );

    let options = [
        "Markdown Note (.md)",
        "Plain Text (.txt)",
        "Drawing (.draw)",
        "Canvas (.canvas)",
    ];
    let items: Vec<ListItem> = options
        .iter()
        .map(|&opt| ListItem::new(Line::from(Span::raw(opt))))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.heading)),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    let state = render_list_with_selection(frame, list, content_area, Some(popup.selected), 0);
    paint_list_hover(
        frame,
        Rect {
            x: content_area.x + 1,
            y: content_area.y + 1,
            width: content_area.width.saturating_sub(2),
            height: content_area.height.saturating_sub(2),
        },
        &state,
        options.len(),
        mouse_pos,
        theme.hover_style(),
    );
}
pub fn draw_hint_bar_style_popup(
    frame: &mut Frame,
    popup: &crate::popups::HintBarStylePopup,
    area: Rect,
    theme: &AppThemeColors,
    keybinds: &crate::keybinds::Keybinds,
    mouse_pos: Option<(u16, u16)>,
) {
    let content_area = draw_popup_frame(
        frame,
        area,
        "HINT BAR STYLE",
        PopupSize::Medium,
        PopupHints::Keybinds(&[
            (
                keybinds.display_list(crate::keybinds::ListAction::MoveUp),
                "up",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::MoveDown),
                "down",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Confirm),
                "select",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Cancel),
                "cancel",
            ),
        ]),
        theme,
    );

    let options = ["Classic", "Sharp", "Rounded", "Slanted"];
    let items: Vec<ListItem> = options
        .iter()
        .map(|&opt| ListItem::new(Line::from(Span::raw(opt))))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.heading)),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    let state = render_list_with_selection(frame, list, content_area, Some(popup.selected), 0);
    paint_list_hover(
        frame,
        Rect {
            x: content_area.x + 1,
            y: content_area.y + 1,
            width: content_area.width.saturating_sub(2),
            height: content_area.height.saturating_sub(2),
        },
        &state,
        options.len(),
        mouse_pos,
        theme.hover_style(),
    );
}
pub fn draw_keybind_preset_popup(
    frame: &mut Frame,
    popup: &crate::popups::KeybindPresetPopup,
    area: Rect,
    theme: &AppThemeColors,
    keybinds: &crate::keybinds::Keybinds,
    mouse_pos: Option<(u16, u16)>,
) {
    let content_area = draw_popup_frame(
        frame,
        area,
        "KEYBIND PRESET",
        PopupSize::Medium,
        PopupHints::Keybinds(&[
            (
                keybinds.display_list(crate::keybinds::ListAction::MoveUp),
                "up",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::MoveDown),
                "down",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Confirm),
                "select",
            ),
            (
                keybinds.display_list(crate::keybinds::ListAction::Cancel),
                "cancel",
            ),
        ]),
        theme,
    );

    let options = [
        "default \u{2014} Default CUA",
        "helix \u{2014} Space leader",
        "vim \u{2014} : commands",
        "emacs \u{2014} Ctrl-x prefix",
    ];
    let items: Vec<ListItem> = options
        .iter()
        .map(|&opt| ListItem::new(Line::from(Span::raw(opt))))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .style(theme.bg_style())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.heading)),
        )
        .highlight_style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );

    let state = render_list_with_selection(frame, list, content_area, Some(popup.selected), 0);
    paint_list_hover(
        frame,
        Rect {
            x: content_area.x + 1,
            y: content_area.y + 1,
            width: content_area.width.saturating_sub(2),
            height: content_area.height.saturating_sub(2),
        },
        &state,
        options.len(),
        mouse_pos,
        theme.hover_style(),
    );
}

pub fn draw_popup_banner(frame: &mut Frame, popup_area: Rect, title: &str, theme: &AppThemeColors) {
    let display_text = format!(" {} ", title.to_uppercase());
    let width = display_text.len() as u16;
    if popup_area.y == 0 {
        return;
    }
    let banner_area = Rect::new(
        popup_area.x + (popup_area.width.saturating_sub(width)) / 2,
        popup_area.y - 1,
        width.min(popup_area.width),
        1,
    );
    frame.render_widget(Clear, banner_area);
    let p = Paragraph::new(Line::from(vec![Span::styled(
        display_text,
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.heading)
            .add_modifier(Modifier::BOLD),
    )]));
    frame.render_widget(p, banner_area);
}

pub fn centered_rect(size: PopupSize, area: Rect) -> Rect {
    let (width_pct, height_pct, max_w, max_h, fixed_h) = match size {
        PopupSize::Small => (40, 40, 60, 20, None),
        PopupSize::Medium => (50, 50, 80, 30, None),
        PopupSize::Large => (60, 60, 100, 40, None),
        PopupSize::Prompt => (50, 0, 80, 0, Some(5)),
        PopupSize::Confirm => (50, 0, 80, 0, Some(12)),
    };

    let width = (area.width * width_pct / 100).clamp(30.min(area.width), max_w.min(area.width));

    let height = if let Some(h) = fixed_h {
        h.min(area.height)
    } else {
        (area.height * height_pct / 100).clamp(5.min(area.height), max_h.min(area.height))
    };

    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

pub fn popup_block<'a>(title: &'a str, theme: &AppThemeColors) -> ratatui::widgets::Block<'a> {
    let mut block = ratatui::widgets::Block::default()
        .style(theme.bg_style())
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(theme.heading));
    if !title.is_empty() {
        block = block.title(title);
    }
    block
}

pub fn build_list_widget<'a>(
    items: impl IntoIterator<Item = ListItem<'a>>,
    theme: &AppThemeColors,
) -> List<'a> {
    List::new(items).highlight_style(
        Style::default()
            .bg(theme.highlight_bg)
            .fg(theme.highlight_fg),
    )
}

/// Map a mouse row to a list item index, honoring scroll offset and row pitch.
///
/// `first_row_y` = screen y of the first item row (for a bordered list, `area.y + 1`;
/// for a borderless list, the list rect's `y`). `row_pitch` = rows per item (1 for
/// single-line, 2 for two-line palette entries). `offset` = the [`ListState::offset()`]
/// captured right after render. `item_count` = total items. Returns `None` when the
/// click is above the first row, in an empty trailing row, or past the last item.
pub fn list_index_at(
    mouse_row: u16,
    first_row_y: u16,
    row_pitch: u16,
    offset: usize,
    item_count: usize,
) -> Option<usize> {
    if item_count == 0 || row_pitch == 0 || mouse_row < first_row_y {
        return None;
    }
    let visual = ((mouse_row - first_row_y) / row_pitch) as usize;
    let idx = visual.saturating_add(offset);
    (idx < item_count).then_some(idx)
}
///
/// Free-scroll a list viewport by `delta` rows, clamped to `[0, max(0, item_count - viewport)]`.
/// `delta < 0` scrolls up. Returns the new offset.
pub fn scroll_viewport(offset: usize, delta: i32, item_count: usize, viewport: usize) -> usize {
    let max_off = item_count.saturating_sub(viewport);
    (offset as i32)
        .saturating_add(delta)
        .clamp(0, max_off as i32) as usize
}
///
/// Clamp `selected` into the visible range `[offset, offset + viewport - 1]` (capped at
/// `item_count - 1`). Returns `0` when `item_count == 0`.
pub fn clamp_selected_to_view(
    selected: usize,
    offset: usize,
    item_count: usize,
    viewport: usize,
) -> usize {
    if item_count == 0 {
        return 0;
    }
    let bottom = (offset + viewport).saturating_sub(1).min(item_count - 1);
    if selected < offset {
        offset
    } else if selected > bottom {
        bottom
    } else {
        selected
    }
}
/// Initialize a [`ListState`] with an optional selection.
pub fn list_state_selected(selected: Option<usize>, offset: usize) -> ListState {
    let mut s = ListState::default().with_offset(offset);
    s.select(selected);
    s
}

/// Paint hover highlight onto the list row under the mouse.
///
/// MUST be called AFTER the list was rendered with `state` (render is what makes
/// `state.offset()` reflect the real scroll). `inner` is the rect where item rows
/// actually live — pass the list block's inner rect (border already removed).
/// Single-row items assumed (index = row - inner.y + offset).
pub fn paint_list_hover(
    frame: &mut Frame,
    inner: Rect,
    state: &ListState,
    item_count: usize,
    mouse_pos: Option<(u16, u16)>,
    hover_style: Style,
) {
    let Some((col, row)) = mouse_pos else {
        return;
    };
    if inner.width == 0 || inner.height == 0 || item_count == 0 {
        return;
    }
    if col < inner.x || col >= inner.x + inner.width {
        return;
    }
    if row < inner.y || row >= inner.y + inner.height {
        return;
    }
    let Some(idx) = list_index_at(row, inner.y, 1, state.offset(), item_count) else {
        return;
    };
    if Some(idx) == state.selected() {
        return;
    }
    let row_rect = Rect {
        x: inner.x,
        y: row,
        width: inner.width,
        height: 1,
    };
    frame.buffer_mut().set_style(row_rect, hover_style);
}

pub fn make_popup_textarea(theme: &AppThemeColors, placeholder: &str) -> TextArea<'static> {
    let mut input = TextArea::default();
    input.set_cursor_line_style(Style::default());
    input.set_style(theme.bg_style());
    if !placeholder.is_empty() {
        input.set_placeholder_text(placeholder);
        input.set_placeholder_style(Style::default().fg(theme.muted));
    }
    input
}

pub fn empty_list_item(theme: &AppThemeColors, label: &str) -> Vec<ListItem<'static>> {
    vec![ListItem::new(Span::styled(
        label.to_string(),
        Style::default().fg(theme.muted),
    ))]
}

pub fn render_list_with_selection(
    frame: &mut Frame,
    list: List,
    area: Rect,
    selected: Option<usize>,
    offset: usize,
) -> ListState {
    let mut state = list_state_selected(selected, offset);
    frame.render_stateful_widget(list, area, &mut state);
    state
}

pub fn unix_ts_to_local(unix_ts: u64) -> chrono::DateTime<chrono::Local> {
    let secs = UNIX_EPOCH + Duration::from_secs(unix_ts);
    secs.into()
}

pub fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let mut t: String = s.chars().take(max_chars).collect();
        t.push('…');
        t
    } else {
        s.to_string()
    }
}

pub fn text_area_from_content(content: &str) -> TextArea<'static> {
    if content.is_empty() {
        TextArea::default()
    } else {
        let lines: Vec<String> = content.lines().map(ToString::to_string).collect();
        TextArea::from(lines)
    }
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

pub fn format_relative_time(unix_ts: u64) -> Cow<'static, str> {
    let now = now_unix_secs();
    let diff = now.saturating_sub(unix_ts);

    if diff < 60 {
        return Cow::Borrowed("just now");
    }
    if diff < 3600 {
        return Cow::Owned(format!("{}m ago", diff / 60));
    }
    if diff < 86_400 {
        return Cow::Owned(format!("{}h ago", diff / 3600));
    }

    let dt = unix_ts_to_local(unix_ts);
    Cow::Owned(dt.format("%Y-%m-%d %H:%M").to_string())
}

pub fn format_date(unix_ts: u64, date_format: &str) -> String {
    let dt = unix_ts_to_local(unix_ts);
    dt.format(date_format).to_string()
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    }
}

pub struct StatusBarBadge {
    pub label: Cow<'static, str>,
    pub style: Style,
}

pub fn ext_badge(enabled: bool, theme: &AppThemeColors) -> StatusBarBadge {
    let label = if enabled { "ext:on" } else { "ext:off" };
    let style = if enabled {
        Style::default()
            .fg(theme.success)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    StatusBarBadge {
        label: format!(" {label} ").into(),
        style,
    }
}

pub fn ext_badge_spans<'a>(
    enabled: bool,
    theme: &AppThemeColors,
    next_bg: Option<Color>,
) -> Vec<Span<'a>> {
    let b = ext_badge(enabled, theme);
    let mut spans = Vec::new();
    match theme.hint_bar_style {
        crate::config::HintBarStyle::Sharp
        | crate::config::HintBarStyle::Rounded
        | crate::config::HintBarStyle::Slanted => {
            let sep_char = match theme.hint_bar_style {
                crate::config::HintBarStyle::Sharp => "\u{e0b0}",
                crate::config::HintBarStyle::Rounded => "\u{e0b4}",
                crate::config::HintBarStyle::Slanted => "\u{e0bc}",
                _ => unreachable!(),
            };
            let pwr_bg = b.style.fg.unwrap_or(theme.accent);
            let pwr_style = Style::default()
                .bg(pwr_bg)
                .fg(theme.highlight_fg)
                .add_modifier(b.style.add_modifier);
            spans.push(Span::styled(b.label, pwr_style));

            let mut sep_style = Style::default().fg(pwr_bg);
            if let Some(bg) = next_bg {
                sep_style = sep_style.bg(bg);
            }
            spans.push(Span::styled(sep_char, sep_style));
        }
        _ => {
            spans.push(Span::styled(b.label, b.style));
            spans.push(Span::raw(" "));
        }
    }
    spans
}

pub fn draw_status_bar<'a>(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    left: Line<'a>,
    right: Option<Line<'a>>,
) {
    if let Some(right_line) = right {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(right_line.width() as u16),
            ])
            .split(area);

        let left_para = Paragraph::new(left).style(theme.hint_line_bg_style());
        frame.render_widget(left_para, chunks[0]);

        let right_para = Paragraph::new(right_line)
            .alignment(Alignment::Right)
            .style(theme.hint_line_bg_style());
        frame.render_widget(right_para, chunks[1]);
    } else {
        let para = Paragraph::new(left).style(theme.hint_line_bg_style());
        frame.render_widget(para, area);
    }
}

/// Always renders hints in the classic muted ` · `-joined style, ignoring `hint_bar_style`.
/// Used by popups so they don't inherit powerline styling.
pub fn format_keybind_hints_classic<'a>(
    theme: &'a AppThemeColors,
    items: &[(String, &'a str)],
) -> Line<'a> {
    let mut spans = Vec::new();
    for (i, (key, action)) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
        }
        spans.push(Span::styled(
            key.clone(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {}", action),
            Style::default().fg(theme.muted),
        ));
    }
    Line::from(spans)
}
pub fn format_keybind_hints<'a>(
    theme: &'a AppThemeColors,
    items: &[(String, &'static str)],
) -> Line<'a> {
    match theme.hint_bar_style {
        crate::config::HintBarStyle::Classic => format_keybind_hints_classic(theme, items),
        style @ (crate::config::HintBarStyle::Sharp
        | crate::config::HintBarStyle::Rounded
        | crate::config::HintBarStyle::Slanted) => {
            let sep_char = match style {
                crate::config::HintBarStyle::Sharp => "\u{e0b0}",
                crate::config::HintBarStyle::Rounded => "\u{e0b4}",
                crate::config::HintBarStyle::Slanted => "\u{e0bc}",
                _ => unreachable!(),
            };

            let bg_colors = [
                theme.accent,
                theme.folder,
                theme.tag,
                theme.warning,
                theme.success,
            ];
            let fg = theme.highlight_fg;
            let mut spans = Vec::new();

            for (i, (key, action)) in items.iter().enumerate() {
                let bg = bg_colors[i % bg_colors.len()];
                let next_bg = if i == items.len() - 1 {
                    theme.hint_line_bg()
                } else {
                    Some(bg_colors[(i + 1) % bg_colors.len()])
                };

                spans.push(Span::styled(
                    format!(" {} {} ", key, action),
                    Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD),
                ));

                let mut sep_style = Style::default().fg(bg);
                if let Some(n_bg) = next_bg {
                    sep_style = sep_style.bg(n_bg);
                }
                spans.push(Span::styled(sep_char, sep_style));
            }
            Line::from(spans)
        }
    }
}

/// Describes the content of a popup footer hint line.
pub enum PopupHints<'a> {
    /// Key-action pairs: each key rendered in accent+bold, actions in muted, joined by ` · `.
    Keybinds(&'a [(String, &'a str)]),
    /// Plain text hint rendered entirely in muted.
    Text(&'a str),
}

/// Build a popup footer `Line` from either keybind pairs or plain text.
/// Always uses classic muted style — never inherits `hint_bar_style`.
pub fn popup_footer_hints<'a>(theme: &'a AppThemeColors, hints: PopupHints<'a>) -> Line<'a> {
    match hints {
        PopupHints::Keybinds(items) => format_keybind_hints_classic(theme, items),
        PopupHints::Text(text) => Line::from(Span::styled(
            text.to_string(),
            Style::default().fg(theme.muted),
        )),
    }
}
pub fn draw_popup_footer(frame: &mut Frame, area: Rect, theme: &AppThemeColors, hints: &Line<'_>) {
    let footer = Paragraph::new(hints.clone())
        .alignment(Alignment::Center)
        .style(theme.hint_line_bg_style());
    frame.render_widget(footer, area);
}

pub fn draw_popup_frame(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    size: PopupSize,
    hints: PopupHints<'_>,
    theme: &AppThemeColors,
) -> Rect {
    let popup_area = centered_rect(size, area);
    frame.render_widget(Clear, popup_area);
    draw_popup_banner(frame, popup_area, title, theme);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(popup_area);
    let hint_line = popup_footer_hints(theme, hints);
    draw_popup_footer(frame, chunks[1], theme, &hint_line);
    chunks[0]
}

pub fn popup_hint_line(theme: &AppThemeColors, text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(theme.muted),
    ))
}

pub fn draw_confirm_popup_frame(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    size: PopupSize,
    is_destructive: bool,
    hints: PopupHints<'_>,
    theme: &AppThemeColors,
) -> Rect {
    let popup_area = centered_rect(size, area);
    frame.render_widget(Clear, popup_area);
    draw_popup_banner(frame, popup_area, title, theme);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(popup_area);

    let hint_line = popup_footer_hints(theme, hints);
    draw_popup_footer(frame, chunks[1], theme, &hint_line);

    let border_color = if is_destructive {
        theme.destructive
    } else {
        theme.heading
    };
    let block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);
    inner
}

pub fn draw_confirm_popup(
    frame: &mut Frame,
    popup: &ConfirmPopup,
    area: Rect,
    theme: &AppThemeColors,
) {
    let inner = draw_confirm_popup_frame(
        frame,
        area,
        "CONFIRM",
        PopupSize::Confirm,
        popup.is_destructive,
        PopupHints::Keybinds(&[
            ("y".to_string(), &popup.confirm_label),
            ("n".to_string(), "cancel"),
        ]),
        theme,
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let message = Paragraph::new(popup.message.as_str()).alignment(Alignment::Center);
    frame.render_widget(message, chunks[0]);

    if let Some(detail) = &popup.detail {
        let detail_para = Paragraph::new(detail.as_str())
            .style(Style::default().fg(theme.muted))
            .alignment(Alignment::Center);
        frame.render_widget(detail_para, chunks[1]);
    }

    let (confirm_style, cancel_style) = if popup.selected_button == 0 {
        let confirm = if popup.is_destructive {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.destructive)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.success)
                .add_modifier(Modifier::BOLD)
        };
        let cancel = Style::default().fg(theme.muted).patch(theme.bg_style());
        (confirm, cancel)
    } else {
        let confirm = if popup.is_destructive {
            Style::default()
                .fg(theme.destructive)
                .patch(theme.bg_style())
        } else {
            Style::default().fg(theme.success).patch(theme.bg_style())
        };
        let cancel = Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.highlight_bg)
            .add_modifier(Modifier::BOLD);
        (confirm, cancel)
    };

    let buttons = Line::from(vec![
        Span::styled(format!(" {} (y) ", popup.confirm_label), confirm_style),
        Span::raw("   "),
        Span::styled(" Cancel (n) ", cancel_style),
    ]);
    let buttons_para = Paragraph::new(buttons).alignment(Alignment::Center);
    frame.render_widget(buttons_para, chunks[3]);
}

pub fn draw_dim_vline(frame: &mut Frame, area: Rect, color: Color) {
    let buf = frame.buffer_mut();
    for row in area.top()..area.bottom() {
        if let Some(cell) = buf.cell_mut((area.x, row)) {
            cell.set_symbol("│");
            cell.set_fg(color);
        }
    }
}

pub fn draw_subnotes_popup(
    frame: &mut Frame,
    popup: &mut crate::popups::SubnotesPopup,
    area: Rect,
    theme: &AppThemeColors,
) {
    let content = draw_popup_frame(
        frame,
        area,
        "SUB-NOTES",
        PopupSize::Large,
        PopupHints::Keybinds(&[
            ("Alt+N".to_string(), "new"),
            ("Ctrl+E".to_string(), "ext edit"),
            ("Esc".to_string(), "back/close"),
            ("Enter/l".to_string(), "edit"),
            ("d/Del".to_string(), "delete"),
            ("Tab/Enter/Shift+Tab".to_string(), "navigate"),
        ]),
        theme,
    );

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(34), Constraint::Min(0)])
        .split(content);

    let list_style = if popup.focus == crate::popups::SubnotesFocus::List {
        Style::default().fg(theme.heading)
    } else {
        Style::default().fg(theme.muted)
    };

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_style(list_style)
        .style(theme.bg_style());

    if popup.subnotes.is_empty() {
        frame.render_widget(list_block.clone(), main_chunks[0]);
        let inner_area = list_block.inner(main_chunks[0]);
        if inner_area.height > 0 {
            let text_area = Rect::new(
                inner_area.x + 2,
                inner_area.y + (inner_area.height.saturating_sub(1) / 2),
                inner_area.width.saturating_sub(4),
                1,
            );
            let placeholder = Paragraph::new("press n to create a new note")
                .style(
                    Style::default()
                        .fg(theme.muted)
                        .add_modifier(Modifier::ITALIC),
                )
                .alignment(Alignment::Center);
            frame.render_widget(placeholder, text_area);
        }
    } else {
        let items: Vec<ListItem> = popup
            .subnotes
            .iter()
            .map(|n| ListItem::new(n.title.as_str()))
            .collect();

        let list = List::new(items)
            .block(list_block)
            .style(theme.bg_style())
            .highlight_style(
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(" ");

        let mut list_state = ListState::default();
        list_state.select(Some(popup.selected));
        frame.render_stateful_widget(list, main_chunks[0], &mut list_state);
        popup.scroll_offset = list_state.offset();
        // Scrollbar for subnotes list
        let sub_list_inner = Rect {
            x: main_chunks[0].x + 1,
            y: main_chunks[0].y + 1,
            width: main_chunks[0].width.saturating_sub(2),
            height: main_chunks[0].height.saturating_sub(2),
        };
        popup.last_scroll = Some(crate::ui::scrollbar::ScrollbarMeta {
            track: crate::ui::scrollbar::track_rect(sub_list_inner),
            content_len: popup.subnotes.len(),
            viewport_len: sub_list_inner.height as usize,
        });
        crate::ui::scrollbar::draw_scrollbar(
            frame,
            sub_list_inner,
            popup.subnotes.len(),
            sub_list_inner.height as usize,
            popup.selected,
            popup.subnotes.len().saturating_sub(1),
            theme,
        );
    }

    let edit_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(main_chunks[1]);

    let title_border_style = if popup.focus == crate::popups::SubnotesFocus::EditTitle {
        Style::default().fg(theme.heading)
    } else {
        Style::default().fg(theme.muted)
    };

    let content_border_style = if popup.focus == crate::popups::SubnotesFocus::EditContent {
        Style::default().fg(theme.heading)
    } else {
        Style::default().fg(theme.muted)
    };

    let mut title_input = popup.title_input.clone();
    title_input.set_placeholder_text("Title...");
    title_input.set_placeholder_style(
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::ITALIC),
    );
    title_input.set_style(theme.bg_style());
    title_input.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(title_border_style)
            .style(theme.bg_style()),
    );
    frame.render_widget(&title_input, edit_chunks[0]);

    let mut content_input = popup.content_input.clone();
    content_input.set_placeholder_text("Content...");
    content_input.set_placeholder_style(
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::ITALIC),
    );
    content_input.set_style(theme.bg_style());
    content_input.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(content_border_style)
            .style(theme.bg_style()),
    );
    frame.render_widget(&content_input, edit_chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    fn has_mod(st: Style, m: Modifier) -> bool {
        st.add_modifier.contains(m)
    }

    #[test]
    fn classic_keybind_hints_pop_key_color() {
        let theme = AppThemeColors::default();
        let line = format_keybind_hints_classic(&theme, &[("j/k".into(), "nav")]);
        let key_span = &line.spans[0];
        assert_eq!(
            key_span.style.fg,
            Some(theme.accent),
            "keybind key fg should be accent color"
        );
        assert!(
            has_mod(key_span.style, Modifier::BOLD),
            "keybind key should be bold"
        );
    }
}
