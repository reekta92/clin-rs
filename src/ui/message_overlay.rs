use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Wrap},
};

use crate::app::messages::MessageSeverity;
use crate::app_theme::AppThemeColors;

/// Render the message overlay: full-width destructive header bar at row 0,
/// centered title, half-width centered dropdown of messages beneath.
/// Long lines wrap. Alternating bg per message (not per line).
pub fn draw_message_overlay(
    frame: &mut Frame,
    app: &crate::app::App,
    theme: &AppThemeColors,
    _area: Rect,
) {
    let messages = &app.messages.messages;
    if messages.is_empty() && !app.messages.force_open {
        return;
    }

    let frame_area = frame.area();
    if frame_area.is_empty() {
        return;
    }

    // --- Dynamic color: destructive (red) for fatal, warning (yellow) for non-fatal ---
    let base_color = if app.messages.has_fatal() {
        theme.destructive
    } else {
        theme.warning
    };

    // --- Title ---
    let title = if app.messages.force_open {
        " [F3] Messages (pinned) "
    } else {
        " [F3] Messages "
    };
    let title = crate::ui::truncate_cells(title, frame_area.width as usize);
    let title_width = unicode_width::UnicodeWidthStr::width(title.as_str()) as u16;

    // --- Full-width header bar at row 0, centered title ---
    let header_rect = Rect::new(frame_area.x, frame_area.y, frame_area.width, 1);
    frame.render_widget(Clear, header_rect);
    frame.render_widget(
        Block::default().style(Style::default().bg(base_color)),
        header_rect,
    );
    let label_x = frame_area.x + (frame_area.width.saturating_sub(title_width)) / 2;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            title.as_str(),
            Style::default().fg(theme.highlight_fg).bg(base_color),
        ))),
        Rect::new(label_x, frame_area.y, title_width, 1),
    );

    // --- No messages, just header ---
    if messages.is_empty() {
        return;
    }

    // --- Filter visible messages: fresh OR force_open ---
    let visible: Vec<_> = messages
        .iter()
        .filter(|m| {
            app.messages.force_open
                || m.severity == MessageSeverity::Fatal
                || crate::app::messages::MessageOverlay::is_fresh(m)
        })
        .collect();
    if visible.is_empty() {
        return;
    }

    // --- Compute popup width: ~half window, centered ---
    let popup_width = crate::ui::header_dropdown_width(frame_area);
    let inner_width = popup_width.saturating_sub(2) as usize;

    // Wrap with the same terminal-cell-aware layout used to paint each message.
    let paragraphs: Vec<_> = visible
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let prefix = match message.severity {
                MessageSeverity::Warning => "⚠ ",
                MessageSeverity::Fatal => "✗ ",
            };
            let text = format!("{prefix}{}", message.text);
            let bg = if index % 2 == 0 {
                base_color
            } else {
                crate::app_theme::darken(base_color, 36)
            };
            let style = Style::default().fg(theme.highlight_fg).bg(bg);
            let line = if let Some(colon) = text.find(':') {
                Line::from(vec![
                    Span::styled(
                        text[..=colon].to_owned(),
                        style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(text[colon + 1..].to_owned(), style),
                ])
            } else {
                Line::from(Span::styled(text, style))
            };
            (
                Paragraph::new(line).style(style).wrap(Wrap { trim: false }),
                style,
            )
        })
        .collect();
    if inner_width == 0 || frame_area.height < 2 {
        return;
    }
    let heights: Vec<_> = paragraphs
        .iter()
        .map(|(p, _)| p.line_count(inner_width as u16))
        .collect();
    let content_len: usize = heights.iter().sum();
    let viewport = 12.min(frame_area.height.saturating_sub(1) as usize);
    let mut skip = app
        .messages
        .scroll
        .min(content_len.saturating_sub(viewport));
    let x = frame_area.x + frame_area.width.saturating_sub(popup_width) / 2;
    let mut y = frame_area.y + 1;
    let bottom = y + viewport as u16;
    for ((paragraph, style), height) in paragraphs.into_iter().zip(heights) {
        if skip >= height {
            skip -= height;
            continue;
        }
        let height = (height - skip).min((bottom - y) as usize) as u16;
        let rect = Rect::new(x, y, popup_width, height);
        frame.render_widget(Clear, rect);
        frame.render_widget(Block::default().style(style), rect);
        frame.render_widget(
            paragraph.scroll((skip as u16, 0)),
            Rect::new(x + 1, y, popup_width.saturating_sub(2), height),
        );
        skip = 0;
        y += height;
        if y >= bottom {
            break;
        }
    }
}
