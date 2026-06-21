use ratatui::{prelude::*, widgets::*};
use crate::app_theme::AppThemeColors;
use super::PreviewHeaderInfo;

pub fn draw_view_title_bar(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    theme: &AppThemeColors,
    preview_info: Option<PreviewHeaderInfo>,
) {
    let display_text = format!(" {} ", title.to_uppercase());
    let title_span = Span::styled(
        display_text,
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.heading)
            .add_modifier(Modifier::BOLD),
    );
    let mut spans = vec![title_span];
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
    let bar = Paragraph::new(Line::from(spans)).style(theme.title_bar_bg_style());
    frame.render_widget(bar, area);
}

pub fn draw_view_title_bar_with_tabs(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    tab_spans: Vec<Span<'static>>,
    theme: &AppThemeColors,
) {
    frame.render_widget(Paragraph::new("").style(theme.title_bar_bg_style()), area);

    let tabs_region = title_bar_tabs_region(area, title);
    let total: u16 = tab_spans
        .iter()
        .map(|s| s.content.chars().count() as u16)
        .fold(0u16, u16::saturating_add);
    let start_x = tabs_region.x + tabs_region.width.saturating_sub(total) / 2;
    let render_w = total.min(tabs_region.width);
    let tabs_area = Rect::new(start_x, area.y, render_w, area.height);
    frame.render_widget(
        Paragraph::new(Line::from(tab_spans)).style(theme.title_bar_bg_style()),
        tabs_area,
    );

    let display_text = format!(" {} ", title.to_uppercase());
    let title_w = display_text.chars().count() as u16;
    let title_area = Rect::new(area.x, area.y, title_w.min(area.width), area.height);
    let title_span = Span::styled(
        display_text,
        Style::default()
            .fg(theme.highlight_fg)
            .bg(theme.heading)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(Paragraph::new(Line::from(vec![title_span])), title_area);
}

pub fn title_bar_tabs_region(area: Rect, title: &str) -> Rect {
    let title_w = format!(" {} ", title.to_uppercase())
        .chars()
        .count()
        .min(area.width as usize) as u16;
    Rect {
        x: area.x + title_w,
        y: area.y,
        width: area.width.saturating_sub(title_w),
        height: area.height,
    }
}

fn tab_display_text(label: &str, glyph: Option<&str>, icons_only: bool) -> String {
    match (icons_only, glyph) {
        (true, Some(g)) => format!(" {g} "),
        (true, None) => format!(" {label} "),
        (false, Some(g)) => format!(" {g} {label} "),
        (false, None) => format!(" {label} "),
    }
}

fn tab_display_width(label: &str, glyph: Option<&str>, icons_only: bool) -> u16 {
    let label_w = label.chars().count() as u16;
    match (icons_only, glyph) {
        (true, Some(g)) => 2 + g.chars().count() as u16, // " g "
        (true, None) => 2 + label_w,                     // " label "
        (false, Some(g)) => 3 + g.chars().count() as u16 + label_w, // " g label "
        (false, None) => 2 + label_w,                    // " label "
    }
}

pub fn build_tab_spans(
    tabs: &[(&str, Option<&str>)],
    active: usize,
    theme: &AppThemeColors,
    icons_only: bool,
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
            tab_display_text(label, *glyph, icons_only),
            style,
        ));
    }
    spans
}

pub fn hit_test_tabs(
    tabs: &[(&str, Option<&str>)],
    region_x: u16,
    region_width: u16,
    click_x: u16,
    icons_only: bool,
) -> Option<usize> {
    let widths: Vec<u16> = tabs
        .iter()
        .map(|(l, g)| tab_display_width(l, *g, icons_only))
        .collect();
    let mut total: u16 = 0;
    for (i, w) in widths.iter().enumerate() {
        total = total.saturating_add(*w);
        if i + 1 < tabs.len() {
            total = total.saturating_add(1); // single-space separator
        }
    }
    let start_x = region_x + region_width.saturating_sub(total) / 2;
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
