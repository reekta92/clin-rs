use super::PreviewHeaderInfo;
use crate::app_theme::AppThemeColors;
use ratatui::{prelude::*, widgets::*};

pub fn draw_view_title_bar(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    theme: &AppThemeColors,
    preview_info: Option<PreviewHeaderInfo>,
    status: Option<&str>,
    right_text: Option<Line<'_>>,
) {
    // Override header when there's an active status notification
    if let Some(st) = status
        && !st.trim().is_empty() && st != "Ready" {
            let st = crate::sanitize::sanitize_for_terminal(st);
            let span = Span::styled(
                format!("  {}  ", st),
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD),
            );
            let bar = Paragraph::new(Line::from(vec![span]))
                .style(Style::default().bg(theme.accent))
                .alignment(Alignment::Center);
            frame.render_widget(bar, area);
            return;
        }
    let display_text = format!(" {} ", title.to_uppercase());
    let title_span = Span::styled(
        display_text,
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.heading)
            .add_modifier(Modifier::BOLD),
    );
    let mut spans = vec![title_span];

    // Powerline separator after title
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
            let sep_style = Style::default()
                .fg(theme.heading)
                .bg(theme.title_bar_bg().unwrap_or(Color::Reset));
            spans.push(Span::styled(sep_char, sep_style));
        }
        _ => {}
    }

    if let Some(info) = preview_info {
        spans.push(Span::styled("  ", Style::default()));
        let parts: Vec<&str> = info.path.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" / ", Style::default().fg(theme.fg)));
            }
            spans.push(Span::styled(
                part.to_string(),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::styled(" ❯ ", Style::default().fg(theme.accent)));
        spans.push(Span::styled(
            info.item_name,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));

        if info.prev_name.is_some() || info.next_name.is_some() {
            spans.push(Span::styled("  (", Style::default().fg(theme.fg)));
            let mut added = false;
            if let Some(prev) = info.prev_name {
                spans.push(Span::styled("prev: ", Style::default().fg(theme.fg)));
                spans.push(Span::styled(prev, Style::default().fg(theme.heading)));
                added = true;
            }
            if let Some(next) = info.next_name {
                if added {
                    spans.push(Span::styled(" · ", Style::default().fg(theme.fg)));
                }
                spans.push(Span::styled("next: ", Style::default().fg(theme.fg)));
                spans.push(Span::styled(next, Style::default().fg(theme.heading)));
            }
            spans.push(Span::styled(")", Style::default().fg(theme.fg)));
        }
    }
    let is_powerline = matches!(
        theme.hint_bar_style,
        crate::config::HintBarStyle::PowerlineSharp
            | crate::config::HintBarStyle::PowerlineRounded
            | crate::config::HintBarStyle::PowerlineSlanted
    );

    let (left_area, right_info) = if let Some(r) = right_text {
        let right_width = r.width() as u16;
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(right_width)])
            .split(area);
        (chunks[0], Some((chunks[1], r)))
    } else {
        (area, None)
    };

    let left_bar = Paragraph::new(Line::from(spans)).style(theme.title_bar_bg_style());
    frame.render_widget(left_bar, left_area);

    if let Some((r_area, r_text)) = right_info {
        if is_powerline {
            // Extract text and split into segments for powerline badges
            let text: String = r_text.spans.iter().map(|s| s.content.as_ref()).collect();
            let segments: Vec<&str> = text.split(" | ").collect();

            let bg_colors = [
                theme.accent,
                theme.folder,
                theme.tag,
                theme.warning,
                theme.success,
            ];
            let sep_char = match theme.hint_bar_style {
                crate::config::HintBarStyle::PowerlineSharp => "",
                crate::config::HintBarStyle::PowerlineRounded => "",
                crate::config::HintBarStyle::PowerlineSlanted => "",
                _ => unreachable!(),
            };

            let mut badge_spans: Vec<Span> = Vec::new();
            for (i, segment) in segments.iter().enumerate() {
                let bg = bg_colors[i % bg_colors.len()];
                let prev_bg = if i == 0 {
                    theme.title_bar_bg()
                } else {
                    Some(bg_colors[(i - 1) % bg_colors.len()])
                };

                // Separator on the left of each badge, bridging from previous element
                let mut sep_style = Style::default().fg(bg);
                if let Some(p_bg) = prev_bg {
                    sep_style = sep_style.bg(p_bg);
                }
                badge_spans.push(Span::styled(sep_char, sep_style));

                badge_spans.push(Span::styled(
                    format!(" {} ", segment.trim()),
                    Style::default()
                        .bg(bg)
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            let r_bar = Paragraph::new(Line::from(badge_spans))
                .style(theme.hint_line_bg_style())
                .alignment(Alignment::Left);
            frame.render_widget(r_bar, r_area);
        } else if theme.hint_bar_style == crate::config::HintBarStyle::Accent {
            let r_bar = Paragraph::new(r_text)
                .style(theme.title_bar_bg_style().fg(theme.accent))
                .alignment(Alignment::Right);
            frame.render_widget(r_bar, r_area);
        } else {
            let r_bar = Paragraph::new(r_text)
                .style(theme.title_bar_bg_style())
                .alignment(Alignment::Right);
            frame.render_widget(r_bar, r_area);
        }
    }
}

pub fn draw_view_title_bar_with_tabs(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    tab_spans: Vec<Span<'static>>,
    theme: &AppThemeColors,
    status: Option<&str>,
    right_text: Option<Line<'_>>,
) {
    // Override header when there's an active status notification
    if let Some(st) = status
        && !st.trim().is_empty() && st != "Ready" {
            let st = crate::sanitize::sanitize_for_terminal(st);
            let span = Span::styled(
                format!("  {}  ", st),
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD),
            );
            let bar = Paragraph::new(Line::from(vec![span]))
                .style(Style::default().bg(theme.accent))
                .alignment(Alignment::Center);
            frame.render_widget(bar, area);
            return;
        }
    frame.render_widget(Paragraph::new("").style(theme.title_bar_bg_style()), area);

    let tabs_region = title_bar_tabs_region(area, title);
    use unicode_width::UnicodeWidthStr;
    let total: u16 = tab_spans
        .iter()
        .map(|s| s.content.width() as u16)
        .fold(0u16, u16::saturating_add);
    let center_x = area.x + area.width.saturating_sub(total) / 2;
    let start_x = center_x.max(tabs_region.x);
    let render_w = total.min(tabs_region.width);
    let tabs_area = Rect::new(start_x, area.y, render_w, area.height);
    frame.render_widget(
        Paragraph::new(Line::from(tab_spans)).style(theme.title_bar_bg_style()),
        tabs_area,
    );

    let display_text = format!(" {} ", title.to_uppercase());
    let title_span = Span::styled(
        display_text,
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.heading)
            .add_modifier(Modifier::BOLD),
    );
    let mut title_spans = vec![title_span];

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
            let sep_style = Style::default()
                .fg(theme.heading)
                .bg(theme.title_bar_bg().unwrap_or(Color::Reset));
            title_spans.push(Span::styled(sep_char, sep_style));
        }
        _ => {}
    }

    let title_w = title_spans
        .iter()
        .map(|s| s.content.width() as u16)
        .sum::<u16>()
        .min(area.width);
    let title_area = Rect::new(area.x, area.y, title_w, area.height);
    frame.render_widget(Paragraph::new(Line::from(title_spans)), title_area);

    // Right-aligned text with hint_bar_style
    if let Some(r_text) = right_text {
        let is_powerline = matches!(
            theme.hint_bar_style,
            crate::config::HintBarStyle::PowerlineSharp
                | crate::config::HintBarStyle::PowerlineRounded
                | crate::config::HintBarStyle::PowerlineSlanted
        );

        if is_powerline {
            let text: String = r_text.spans.iter().map(|s| s.content.as_ref()).collect();
            let segments: Vec<&str> = text.split(" | ").collect();

            let bg_colors = [
                theme.accent,
                theme.folder,
                theme.tag,
                theme.warning,
                theme.success,
            ];
            let sep_char = match theme.hint_bar_style {
                crate::config::HintBarStyle::PowerlineSharp => "",
                crate::config::HintBarStyle::PowerlineRounded => "",
                crate::config::HintBarStyle::PowerlineSlanted => "",
                _ => unreachable!(),
            };

            let mut badge_spans: Vec<Span> = Vec::new();
            for (i, segment) in segments.iter().enumerate() {
                let bg = bg_colors[i % bg_colors.len()];
                let prev_bg = if i == 0 {
                    theme.title_bar_bg()
                } else {
                    Some(bg_colors[(i - 1) % bg_colors.len()])
                };

                let mut sep_style = Style::default().fg(bg);
                if let Some(p_bg) = prev_bg {
                    sep_style = sep_style.bg(p_bg);
                }
                badge_spans.push(Span::styled(sep_char, sep_style));

                badge_spans.push(Span::styled(
                    format!(" {} ", segment.trim()),
                    Style::default()
                        .bg(bg)
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            frame.render_widget(
                Paragraph::new(Line::from(badge_spans)).alignment(Alignment::Right),
                area,
            );
        } else if theme.hint_bar_style == crate::config::HintBarStyle::Accent {
            frame.render_widget(
                Paragraph::new(r_text)
                    .style(Style::default().fg(theme.accent))
                    .alignment(Alignment::Right),
                area,
            );
        } else {
            frame.render_widget(Paragraph::new(r_text).alignment(Alignment::Right), area);
        }
    }
}

pub fn title_bar_tabs_region(area: Rect, title: &str) -> Rect {
    use unicode_width::UnicodeWidthStr;
    let title_w = format!(" {} ", title.to_uppercase())
        .width()
        .min(area.width as usize) as u16;
    Rect {
        x: area.x + title_w,
        y: area.y,
        width: area.width.saturating_sub(title_w),
        height: area.height,
    }
}

fn tab_display_text(
    label: &str,
    glyph: Option<&str>,
    icons_only: bool,
    icon_mode: crate::config::IconMode,
) -> String {
    let effective_icons_only = icons_only && icon_mode != crate::config::IconMode::None;
    let effective_glyph = match icon_mode {
        crate::config::IconMode::None => None,
        _ => glyph,
    };
    match (effective_icons_only, effective_glyph) {
        (true, Some(g)) => format!(" {g} "),
        (true, None) => format!(" {label} "),
        (false, Some(g)) => format!(" {g} {label} "),
        (false, None) => format!(" {label} "),
    }
}

fn tab_display_width(
    label: &str,
    glyph: Option<&str>,
    icons_only: bool,
    icon_mode: crate::config::IconMode,
) -> u16 {
    use unicode_width::UnicodeWidthStr;
    let label_w = label.width() as u16;
    let effective_icons_only = icons_only && icon_mode != crate::config::IconMode::None;
    let effective_glyph = match icon_mode {
        crate::config::IconMode::None => None,
        _ => glyph,
    };
    match (effective_icons_only, effective_glyph) {
        (true, Some(g)) => 2 + g.width() as u16,
        (true, None) => 2 + label_w,
        (false, Some(g)) => 3 + g.width() as u16 + label_w,
        (false, None) => 2 + label_w,
    }
}

pub fn build_tab_spans(
    tabs: &[(&str, Option<&str>)],
    active: usize,
    theme: &AppThemeColors,
    icons_only: bool,
    icon_mode: crate::config::IconMode,
) -> Vec<Span<'static>> {
    let active_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let inactive_style = Style::default()
        .fg(theme.muted)
        .add_modifier(Modifier::BOLD);
    let mut spans = Vec::with_capacity(tabs.len() * 2);
    for (i, (label, glyph)) in tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let style = if i == active {
            active_style
        } else {
            inactive_style
        };
        spans.push(Span::styled(
            tab_display_text(label, *glyph, icons_only, icon_mode),
            style,
        ));
    }
    spans
}

pub fn hit_test_tabs(
    tabs: &[(&str, Option<&str>)],
    area_x: u16,
    area_width: u16,
    min_x: u16,
    click_x: u16,
    icons_only: bool,
    icon_mode: crate::config::IconMode,
) -> Option<usize> {
    let widths: Vec<u16> = tabs
        .iter()
        .map(|(l, g)| tab_display_width(l, *g, icons_only, icon_mode))
        .collect();
    let mut total: u16 = 0;
    for (i, w) in widths.iter().enumerate() {
        total = total.saturating_add(*w);
        if i + 1 < tabs.len() {
            total = total.saturating_add(1); // single-space separator
        }
    }
    let center_x = area_x + area_width.saturating_sub(total) / 2;
    let start_x = center_x.max(min_x);
    if click_x < start_x || click_x >= start_x.saturating_add(total) {
        return None;
    }
    let mut offset = start_x;
    for (i, w) in widths.iter().enumerate() {
        if click_x < offset.saturating_add(*w) {
            return Some(i);
        }
        offset = offset.saturating_add(*w).saturating_add(1);
    }
    None
}
