use ratatui::{prelude::*, widgets::*};
use ratatui_textarea::TextArea;
use std::borrow::Cow;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{PopupSize, get_textarea_scroll};
use crate::app::{ConfirmPopup, TemplatePopup, ThemePopup};
use crate::app_theme::AppThemeColors;

pub fn draw_template_popup(
    frame: &mut Frame,
    popup: &TemplatePopup,
    area: Rect,
    theme: &AppThemeColors,
) {
    let hint_line = popup_hint_line(
        theme,
        "Tab switch · Enter use template · n create · d delete · Space edit · ? help · Esc cancel",
    );
    let content = draw_popup_frame(
        frame,
        area,
        "TEMPLATES",
        PopupSize::Large,
        &hint_line,
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
        vec![ListItem::new(Span::styled(
            "(no matching templates)",
            Style::default().fg(theme.muted),
        ))]
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
    );

    frame.render_stateful_widget(list, chunks[1], &mut state);
}

pub fn draw_theme_popup(frame: &mut Frame, popup: &ThemePopup, area: Rect, theme: &AppThemeColors) {
    let hint_line = popup_hint_line(theme, "Tab navigate · Enter select · Esc close");
    let content = draw_popup_frame(frame, area, "THEMES", PopupSize::Medium, &hint_line, theme);

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

    let mut state = list_state_selected(Some(popup.selected));
    frame.render_stateful_widget(list, chunks[0], &mut state);

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

    let gen_style = if popup.general_is_solid {
        Style::default().fg(theme.success)
    } else {
        Style::default().fg(theme.destructive)
    };
    let gen_block = Block::default()
        .style(theme.bg_style())
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
    let graph_block = Block::default()
        .style(theme.bg_style())
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
) {
    let hint_line = popup_hint_line(theme, "↑↓: Navigate • Enter: Select • Esc: Cancel");
    let content_area =
        draw_popup_frame(frame, area, "SORT BY", PopupSize::Medium, &hint_line, theme);

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

    let mut state = list_state_selected(Some(popup.selected));
    frame.render_stateful_widget(list, content_area, &mut state);
}

pub fn draw_icon_mode_popup(
    frame: &mut Frame,
    popup: &crate::popups::IconModePopup,
    area: Rect,
    theme: &AppThemeColors,
) {
    let hint_line = popup_hint_line(theme, "↑↓: Navigate • Enter: Select • Esc: Cancel");
    let content_area = draw_popup_frame(
        frame,
        area,
        "ICON MODE",
        PopupSize::Medium,
        &hint_line,
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

    let mut state = list_state_selected(Some(popup.selected));
    frame.render_stateful_widget(list, content_area, &mut state);
}

pub fn draw_create_format_popup(
    frame: &mut Frame,
    popup: &crate::popups::CreateFormatPopup,
    area: Rect,
    theme: &AppThemeColors,
) {
    let hint_line = popup_hint_line(theme, "↑↓: Navigate • Enter: Select • Esc: Cancel");
    let content_area = draw_popup_frame(
        frame,
        area,
        "CREATE NEW",
        PopupSize::Medium,
        &hint_line,
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

    let mut state = list_state_selected(Some(popup.selected));
    frame.render_stateful_widget(list, content_area, &mut state);
}

pub fn draw_hint_bar_style_popup(
    frame: &mut Frame,
    popup: &crate::popups::HintBarStylePopup,
    area: Rect,
    theme: &AppThemeColors,
) {
    let hint_line = popup_hint_line(theme, "↑↓: Navigate • Enter: Select • Esc: Cancel");
    let content_area = draw_popup_frame(
        frame,
        area,
        "HINT BAR STYLE",
        PopupSize::Medium,
        &hint_line,
        theme,
    );

    let options = [
        "Classic",
        "Accent",
        "Powerline Sharp",
        "Powerline Rounded",
        "Powerline Slanted",
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

    let mut state = list_state_selected(Some(popup.selected));
    frame.render_stateful_widget(list, content_area, &mut state);
}

pub fn draw_keybind_preset_popup(
    frame: &mut Frame,
    popup: &crate::popups::KeybindPresetPopup,
    area: Rect,
    theme: &AppThemeColors,
) {
    let hint_line = popup_hint_line(
        theme,
        "\u{2191}\u{2193}: Navigate \u{2022} Enter: Select \u{2022} Esc: Cancel",
    );
    let content_area = draw_popup_frame(
        frame,
        area,
        "KEYBIND PRESET",
        PopupSize::Medium,
        &hint_line,
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

    let mut state = list_state_selected(Some(popup.selected));
    frame.render_stateful_widget(list, content_area, &mut state);
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

/// Initialize a [`ListState`] with an optional selection.
pub fn list_state_selected(selected: Option<usize>) -> ListState {
    let mut s = ListState::default();
    s.select(selected);
    s
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

    let secs = UNIX_EPOCH + Duration::from_secs(unix_ts);
    let dt: chrono::DateTime<chrono::Local> = secs.into();
    Cow::Owned(dt.format("%Y-%m-%d %H:%M").to_string())
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

pub fn draw_status_bar<'a>(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    badge: Option<StatusBarBadge>,
    hint: Line<'a>,
    right: Option<Line<'a>>,
    pending: Option<&str>,
) {
    let mut left_spans: Vec<Span> = Vec::new();
    if let Some(p) = pending {
        left_spans.push(Span::styled(
            format!("{p} "),
            Style::default().fg(theme.highlight_fg).bg(theme.accent),
        ));
    }
    if let Some(b) = badge {
        match theme.hint_bar_style {
            crate::config::HintBarStyle::PowerlineSharp
            | crate::config::HintBarStyle::PowerlineRounded
            | crate::config::HintBarStyle::PowerlineSlanted => {
                let sep_char = match theme.hint_bar_style {
                    crate::config::HintBarStyle::PowerlineSharp => "",
                    crate::config::HintBarStyle::PowerlineRounded => "",
                    crate::config::HintBarStyle::PowerlineSlanted => "",
                    _ => unreachable!(),
                };
                let pwr_bg = b.style.fg.unwrap_or(theme.accent);
                let pwr_style = Style::default()
                    .bg(pwr_bg)
                    .fg(theme.highlight_fg)
                    .add_modifier(b.style.add_modifier);
                left_spans.push(Span::styled(b.label, pwr_style));

                let next_bg = hint
                    .spans
                    .first()
                    .and_then(|s| s.style.bg)
                    .or(theme.hint_line_bg());
                let mut sep_style = Style::default().fg(pwr_bg);
                if let Some(bg) = next_bg {
                    sep_style = sep_style.bg(bg);
                }
                left_spans.push(Span::styled(sep_char, sep_style));
            }
            _ => {
                left_spans.push(Span::styled(b.label, b.style));
                left_spans.push(Span::raw(" "));
            }
        }
    }
    left_spans.extend(hint.spans);

    if let Some(right_line) = right {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(right_line.width() as u16),
            ])
            .split(area);

        let left_para = Paragraph::new(Line::from(left_spans)).style(theme.hint_line_bg_style());
        frame.render_widget(left_para, chunks[0]);

        let right_para = Paragraph::new(right_line)
            .alignment(Alignment::Right)
            .style(theme.hint_line_bg_style());
        frame.render_widget(right_para, chunks[1]);
    } else {
        let para = Paragraph::new(Line::from(left_spans)).style(theme.hint_line_bg_style());
        frame.render_widget(para, area);
    }
}

pub fn format_keybind_hints<'a>(
    theme: &'a AppThemeColors,
    items: &[(String, &'static str)],
) -> Line<'a> {
    match theme.hint_bar_style {
        crate::config::HintBarStyle::Classic => {
            let mut spans = Vec::new();
            for (i, (key, action)) in items.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
                }
                spans.push(Span::styled(
                    key.clone(),
                    Style::default()
                        .fg(theme.muted)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!(" {}", action),
                    Style::default().fg(theme.muted),
                ));
            }
            Line::from(spans)
        }
        crate::config::HintBarStyle::Accent => {
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
        style @ (crate::config::HintBarStyle::PowerlineSharp
        | crate::config::HintBarStyle::PowerlineRounded
        | crate::config::HintBarStyle::PowerlineSlanted) => {
            let sep_char = match style {
                crate::config::HintBarStyle::PowerlineSharp => "",
                crate::config::HintBarStyle::PowerlineRounded => "",
                crate::config::HintBarStyle::PowerlineSlanted => "",
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
    hints: &Line<'_>,
    theme: &AppThemeColors,
) -> Rect {
    let popup_area = centered_rect(size, area);
    frame.render_widget(Clear, popup_area);
    draw_popup_banner(frame, popup_area, title, theme);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(popup_area);
    draw_popup_footer(frame, chunks[1], theme, hints);
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
    theme: &AppThemeColors,
) -> Rect {
    let popup_area = centered_rect(size, area);
    frame.render_widget(Clear, popup_area);
    draw_popup_banner(frame, popup_area, title, theme);
    let border_color = if is_destructive {
        theme.destructive
    } else {
        theme.heading
    };
    let block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
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

pub fn draw_corner_watermark(frame: &mut Frame, area: Rect, color: Color) {
    let version = env!("CARGO_PKG_VERSION");
    let text = format!("clin v{version}");
    let width = text.len() as u16;
    if area.width < width + 2 || area.height < 1 {
        return;
    }
    let wm_area = Rect::new(area.x + area.width - width - 1, area.y, width, 1);
    let para = Paragraph::new(text).style(Style::default().fg(color));
    frame.render_widget(para, wm_area);
}

pub fn fill_cursor_line_bg(frame: &mut Frame, editor: &TextArea, area: Rect, bg: Color) {
    if editor.selection_range().is_some() {
        return;
    }
    let (scroll_row, _) = get_textarea_scroll(editor);
    let cursor_row = editor.cursor().0;
    let screen_row = cursor_row.saturating_sub(scroll_row) as u16;
    let inner_y = editor.block().map(|b| b.inner(area).y).unwrap_or(area.y);
    let y = inner_y + screen_row;
    if y < area.y || y >= area.bottom() {
        return;
    }
    let buf = frame.buffer_mut();
    for x in area.left()..area.right() {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_bg(bg);
        }
    }
}

pub fn draw_subnotes_popup(
    frame: &mut Frame,
    popup: &crate::popups::SubnotesPopup,
    area: Rect,
    theme: &AppThemeColors,
) {
    let hint_line = popup_hint_line(
        theme,
        "Alt+N new · Ctrl+E ext edit · Esc back/close · Enter/l edit · d/Del delete · Tab/Enter/Shift+Tab navigate",
    );
    let content = draw_popup_frame(
        frame,
        area,
        "SUB-NOTES",
        PopupSize::Large,
        &hint_line,
        theme,
    );

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(34),
            Constraint::Min(0),
        ])
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
                .style(Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC))
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
    }

    let edit_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ])
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
