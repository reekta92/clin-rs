use std::collections::HashMap;
use std::time::{Duration, UNIX_EPOCH};

use chrono::Datelike;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::app_theme::AppThemeColors;
use crate::storage::NoteSummary;

/// Draw a single-month calendar (current month) with note activity at the bottom
/// of the notes view.
///
/// Days that have at least one note — by modification date, since that is the
/// only timestamp on [`NoteSummary`] — are marked with a leading `•`; today is
/// highlighted with the accent color on the success background. The calendar
/// reflects exactly the notes it is given (the active, non-trashed list); no
/// extra filtering is applied here.
///
/// Needs at least 9 rows (1 top divider + 1 title + 1 weekday header + 6 week
/// rows) and 7 columns; otherwise it no-ops to avoid clipping or panics.
pub fn draw_calendar(frame: &mut Frame, rect: Rect, theme: &AppThemeColors, notes: &[NoteSummary]) {
    if rect.height < 9 || rect.width < 7 {
        return;
    }

    let today = chrono::Local::now().date_naive();
    let year = today.year();
    let month = today.month();
    let today_day = today.day();

    // Aggregate note activity for the current month (modified-date metric).
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for n in notes {
        let secs = UNIX_EPOCH + Duration::from_secs(n.updated_at);
        let dt: chrono::DateTime<chrono::Local> = secs.into();
        let d = dt.date_naive();
        if d.month() == month && d.year() == year {
            *counts.entry(d.day()).or_insert(0) += 1;
        }
    }

    // First weekday of the month (0 = Sunday, matching the Su..Sa header) and
    // the number of days in the month.
    let first = chrono::NaiveDate::from_ymd_opt(year, month, 1)
        .expect("first day of any month is a valid date");
    let lead = first.weekday().num_days_from_sunday() as usize;
    let mut days_in_month = 0u32;
    while chrono::NaiveDate::from_ymd_opt(year, month, days_in_month + 1).is_some() {
        days_in_month += 1;
    }

    let mut lines: Vec<Line> = Vec::with_capacity(8);

    // Title line.
    lines.push(Line::from(vec![Span::styled(
        format!("  {}", today.format("%B %Y")),
        Style::default()
            .fg(theme.heading)
            .add_modifier(Modifier::BOLD),
    )]));

    // Weekday header — fixed 3-char columns, Sunday-first.
    lines.push(Line::from(vec![Span::styled(
        " Su Mo Tu We Th Fr Sa",
        Style::default().fg(theme.muted),
    )]));

    // 6 week rows × 7 columns. Leading/trailing out-of-month slots render as
    // blank 3-char cells so the columns stay aligned.
    for week in 0..6u32 {
        let mut spans: Vec<Span> = Vec::with_capacity(7);
        for col in 0..7u32 {
            let day = (week * 7 + col) as i32 - lead as i32 + 1;
            if day < 1 || day > days_in_month as i32 {
                spans.push(Span::styled("   ", Style::default().fg(theme.muted)));
                continue;
            }
            let day_u = day as u32;
            let count = counts.get(&day_u).copied().unwrap_or(0);
            let (prefix, style) = if day_u == today_day {
                (
                    '\u{2022}',
                    Style::default()
                        .fg(theme.accent)
                        .bg(theme.success)
                        .add_modifier(Modifier::BOLD),
                )
            } else if count > 0 {
                ('\u{2022}', Style::default().fg(theme.success))
            } else {
                (' ', Style::default().fg(theme.muted))
            };
            spans.push(Span::styled(format!("{prefix}{day:>2}"), style));
        }
        lines.push(Line::from(spans));
    }

    let block = Block::default()
        .style(theme.bg_style())
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme.muted))
        .padding(Padding::new(2, 2, 0, 0));
    let paragraph = Paragraph::new(lines).style(theme.bg_style()).block(block);
    frame.render_widget(paragraph, rect);
}
