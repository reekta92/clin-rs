use super::PreviewHeaderInfo;
use crate::app_theme::AppThemeColors;
use ratatui::{prelude::*, widgets::*};

fn spinner_char(tick: usize) -> char {
    const FRAMES: [char; 10] = [
        '\u{2801}', '\u{2802}', '\u{2804}', '\u{2840}', '\u{2844}', '\u{2848}', '\u{2850}',
        '\u{2860}', '\u{28C0}', '\u{28C4}',
    ];
    FRAMES[tick % FRAMES.len()]
}

#[allow(clippy::too_many_arguments)]
pub fn draw_view_title_bar(
    frame: &mut Frame,
    area: Rect,
    theme: &AppThemeColors,
    left: Line<'_>,
    right: Option<Line<'_>>,
    status: Option<&str>,
    tick: usize,
) {
    // Override header when there's an active status notification
    if let Some(st) = status
        && !st.trim().is_empty()
        && st != "Ready"
    {
        let st = crate::sanitize::sanitize_for_terminal(st);
        let display = if st.starts_with("First time caching") {
            format!("  {} {}  ", spinner_char(tick), st)
        } else {
            format!("  {}  ", st)
        };
        let span = Span::styled(
            display,
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

    let (left_area, right_info) = if let Some(r) = right {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(r.width() as u16)])
            .split(area);
        (chunks[0], Some((chunks[1], r)))
    } else {
        (area, None)
    };

    let left_bar = Paragraph::new(left).style(theme.title_bar_bg_style());
    frame.render_widget(left_bar, left_area);

    if let Some((r_area, r_text)) = right_info {
        let is_powerline = matches!(
            theme.hint_bar_style,
            crate::config::HintBarStyle::Sharp
                | crate::config::HintBarStyle::Rounded
                | crate::config::HintBarStyle::Slanted
        );
        if is_powerline {
            let r_bar = Paragraph::new(r_text)
                .style(theme.hint_line_bg_style())
                .alignment(Alignment::Left);
            frame.render_widget(r_bar, r_area);
        } else {
            let r_bar = Paragraph::new(r_text)
                .style(theme.title_bar_bg_style())
                .alignment(Alignment::Right);
            frame.render_widget(r_bar, r_area);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_view_title_bar_with_tabs(
    frame: &mut Frame,
    area: Rect,
    default_title: &str,
    theme: &AppThemeColors,
    left: Line<'_>,
    tab_spans: Vec<Span<'static>>,
    right: Option<Line<'_>>,
    status: Option<&str>,
    tick: usize,
) {
    // Override header when there's an active status notification
    if let Some(st) = status
        && !st.trim().is_empty()
        && st != "Ready"
    {
        let st = crate::sanitize::sanitize_for_terminal(st);
        let display = if st.starts_with("First time caching") {
            format!("  {} {}  ", spinner_char(tick), st)
        } else {
            format!("  {}  ", st)
        };
        let span = Span::styled(
            display,
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

    let tabs_region = title_bar_tabs_region(area, default_title);
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

    let title_w = left.width() as u16;
    let title_area = Rect::new(area.x, area.y, title_w, area.height);
    frame.render_widget(Paragraph::new(left), title_area);

    if let Some(r_text) = right {
        let right_start = title_area.right().max(tabs_area.right());
        let right_width = area.right().saturating_sub(right_start);
        if right_width > 0 {
            let right_rect = Rect::new(right_start, area.y, right_width, area.height);
            frame.render_widget(
                Paragraph::new(r_text).alignment(Alignment::Right),
                right_rect,
            );
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
    hovered: Option<usize>,
    theme: &AppThemeColors,
    icons_only: bool,
    icon_mode: crate::config::IconMode,
) -> Vec<Span<'static>> {
    let active_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let inactive_style = Style::default().fg(theme.muted);
    let mut spans = Vec::with_capacity(tabs.len() * 2);
    for (i, (label, glyph)) in tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let style = if i == active {
            active_style
        } else if Some(i) == hovered {
            theme.hover_style()
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

pub fn preview_spans<'a>(info: &PreviewHeaderInfo, theme: &AppThemeColors) -> Vec<Span<'a>> {
    let mut spans = Vec::new();
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
        info.item_name.clone(),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ));

    if info.prev_name.is_some() || info.next_name.is_some() {
        spans.push(Span::styled("  (", Style::default().fg(theme.fg)));
        let mut added = false;
        if let Some(prev) = &info.prev_name {
            spans.push(Span::styled("prev: ", Style::default().fg(theme.fg)));
            spans.push(Span::styled(
                prev.clone(),
                Style::default().fg(theme.heading),
            ));
            added = true;
        }
        if let Some(next) = &info.next_name {
            if added {
                spans.push(Span::styled(" · ", Style::default().fg(theme.fg)));
            }
            spans.push(Span::styled("next: ", Style::default().fg(theme.fg)));
            spans.push(Span::styled(
                next.clone(),
                Style::default().fg(theme.heading),
            ));
        }
        spans.push(Span::styled(")", Style::default().fg(theme.fg)));
    }
    spans
}
