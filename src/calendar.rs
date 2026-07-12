use std::collections::HashMap;

use chrono::Datelike;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::app_theme::AppThemeColors;
use crate::storage::NoteSummary;

/// Draw a GitHub-contributions-style rolling-weeks heatmap at the bottom of
/// the notes view.
///
/// Columns are weeks ending at the current day; rows are weekdays. Each cell
/// is shaded by the count of active (non-trashed) notes modified on that day
/// (using the modification timestamp on [`NoteSummary`]). Today is highlighted
/// with a filled cell.
///
/// Needs at least 9 rows and 8 columns; otherwise it no-ops.
pub fn draw_calendar(
    frame: &mut Frame,
    rect: Rect,
    theme: &AppThemeColors,
    notes: &[NoteSummary],
    bottom_border: bool,
    week_start: crate::config::WeekStart,
    strip_rect: Rect,
) {
    if rect.height < 9 || rect.width < 8 {
        return;
    }

    let today = chrono::Local::now().date_naive();

    // Width-adaptive week count.
    const LEFT_LABEL: u16 = 3; // "Mo "
    const COL_PITCH: u16 = 2; // cell char + gap
    let inner_w = rect.width.saturating_sub(4 + LEFT_LABEL); // 4 = block padding (2+2)
    let weeks = (inner_w / COL_PITCH).clamp(1, 26);

    // Window: last column starts on the week-start weekday, ending with today.
    let today_wd = today.weekday().num_days_from_sunday() as i64;
    let target = match week_start {
        crate::config::WeekStart::Sunday => 0,
        crate::config::WeekStart::Monday => 1,
    };
    let shift = (today_wd - target).rem_euclid(7);
    let last_col_start = today - chrono::Duration::days(shift);
    let first_col_start = last_col_start - chrono::Duration::days(7 * (weeks as i64 - 1));

    // Aggregate counts per date within the window.
    let mut counts: HashMap<chrono::NaiveDate, usize> = HashMap::new();
    for n in notes {
        let dt = crate::ui::unix_ts_to_local(n.updated_at);
        let d = dt.date_naive();
        if d >= first_col_start && d <= today {
            *counts.entry(d).or_insert(0) += 1;
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(8);

    // Month-labels row (GitHub-style).
    let mut month_row_text = String::from("   "); // align with day-label column
    let mut prev_month = 0u32;
    for col_i in 0..weeks as usize {
        let col_start = first_col_start + chrono::Duration::days(7 * col_i as i64);
        let m = col_start.month();
        if col_i == 0 || m != prev_month {
            month_row_text.push_str(&format!("{:<3}", col_start.format("%b")));
            prev_month = m;
        } else {
            month_row_text.push_str("  ");
        }
    }
    let expected_row_w = (LEFT_LABEL + weeks * COL_PITCH) as usize;
    month_row_text.truncate(expected_row_w);
    lines.push(Line::from(Span::raw(month_row_text)));

    // 7 day-rows.
    let weekdays: &[&str] = match week_start {
        crate::config::WeekStart::Sunday => &["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
        crate::config::WeekStart::Monday => &["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    };

    for (row_i, weekday) in weekdays.iter().enumerate() {
        let mut spans: Vec<Span> = Vec::with_capacity(weeks as usize + 1);
        // Day-label prefix.
        spans.push(Span::styled(
            format!("{} ", weekday),
            Style::default().fg(theme.muted),
        ));
        for col_i in 0..weeks as usize {
            let date = first_col_start + chrono::Duration::days((7 * col_i + row_i) as i64);
            let count = counts.get(&date).copied().unwrap_or(0);
            let (ch, style) = if date == today {
                (
                    '\u{2588}', // █
                    Style::default()
                        .fg(theme.highlight_fg)
                        .bg(theme.highlight_bg)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                match count {
                    0 => ('\u{00B7}', Style::default().fg(theme.muted)), // ·
                    1 => ('\u{2591}', Style::default().fg(theme.text)),  // ░
                    2..=3 => ('\u{2592}', Style::default().fg(theme.accent)), // ▒
                    _ => (
                        '\u{2593}', // ▓
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                }
            };
            spans.push(Span::styled(format!("{ch} "), style));
        }
        lines.push(Line::from(spans));
    }

    // Border at the interface edge spans the full strip width so that a single
    // centered section still gets a full-width border.
    let border = if bottom_border {
        Borders::BOTTOM
    } else {
        Borders::TOP
    };
    let border_bg = theme.bg.unwrap_or(Color::Reset);
    let strip_block = Block::default()
        .style(theme.bg_style())
        .borders(border)
        .border_style(Style::default().fg(theme.muted).bg(border_bg));
    let inner = strip_block.inner(strip_rect);
    frame.render_widget(&strip_block, strip_rect);

    // Content area = section rect clipped to the border's inner area.
    let content_x = rect.x.max(inner.x);
    let content_y = rect.y.max(inner.y);
    let content_w = (rect.right().min(inner.right())).saturating_sub(content_x);
    let content_h = (rect.bottom().min(inner.bottom())).saturating_sub(content_y);
    let content_area = Rect::new(content_x, content_y, content_w, content_h);
    let pad_top = content_area.height.saturating_sub(8) / 2;
    let inner_block = Block::default()
        .style(theme.bg_style())
        .padding(Padding::new(2, 2, pad_top, 0));
    let paragraph = Paragraph::new(lines)
        .style(theme.bg_style())
        .block(inner_block);
    frame.render_widget(paragraph, content_area);
}
