use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
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
    let title_width = title.chars().count() as u16;

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
            title,
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
    let popup_width = (frame_area.width / 2).clamp(30, 80);
    let inner_width = popup_width.saturating_sub(2) as usize;

    // --- Pre-wrap messages, tracking which message each line belongs to ---
    struct WrappedLine {
        text: String,
        fg: Color,
        msg_idx: usize, // which message this line belongs to
    }

    let prefix_chars: usize = 2;
    let mut wrapped_lines: Vec<WrappedLine> = Vec::new();

    for (msg_idx, m) in visible.iter().enumerate() {
        let fg = theme.highlight_fg;
        let prefix = match m.severity {
            MessageSeverity::Warning => "⚠ ",
            MessageSeverity::Fatal => "✗ ",
        };
        let full_text = format!("{}{}", prefix, m.text);
        let text_chars = full_text.chars().count();

        if text_chars <= inner_width {
            wrapped_lines.push(WrappedLine {
                text: full_text,
                fg,
                msg_idx,
            });
        } else {
            let first_width = inner_width;
            let cont_width = inner_width.saturating_sub(prefix_chars);
            let chars: Vec<char> = full_text.chars().collect();
            let mut offset = 0;
            let mut first = true;
            while offset < chars.len() {
                let width = if first { first_width } else { cont_width };
                let end = (offset + width).min(chars.len());
                let segment: String = chars[offset..end].iter().collect();
                let display = if first {
                    segment
                } else {
                    format!("  {}", segment)
                };
                wrapped_lines.push(WrappedLine {
                    text: display,
                    fg,
                    msg_idx,
                });
                offset = end;
                first = false;
            }
        }
    }

    let max_visible_rows = 12;
    let max_scroll = wrapped_lines.len().saturating_sub(max_visible_rows);
    let scroll = app.messages.scroll.min(max_scroll);
    let total_rows = wrapped_lines.len().min(max_visible_rows);

    // --- Center the dropdown horizontally ---
    let x = frame_area.x + (frame_area.width.saturating_sub(popup_width)) / 2;
    let height = (total_rows as u16).min(frame_area.height.saturating_sub(2).max(1));
    let dropdown_area = Rect::new(x, frame_area.y + 1, popup_width, height);

    frame.render_widget(Clear, dropdown_area);
    frame.render_widget(
        Block::default().style(Style::default().bg(base_color)),
        dropdown_area,
    );
    // --- Render lines: alternating bg per MESSAGE, not per line ---
    let alt_bg = darken(base_color, 36);

    for (i, wl) in wrapped_lines.iter().skip(scroll).take(height as usize).enumerate() {
        // Alternate based on message index: even msg_idx → base_color, odd → darkened
        let bg = if wl.msg_idx % 2 == 0 {
            base_color
        } else {
            alt_bg
        };
        let row_y = dropdown_area.y + i as u16;

        let full_row = Rect::new(dropdown_area.x, row_y, popup_width, 1);
        frame.render_widget(Clear, full_row);
        frame.render_widget(Block::default().style(Style::default().bg(bg)), full_row);

        let styled_line = Line::from(Span::styled(
            &wl.text,
            Style::default().fg(wl.fg).bg(bg),
        ));
        let content_area =
            Rect::new(dropdown_area.x + 1, row_y, popup_width.saturating_sub(2), 1);
        frame.render_widget(Paragraph::new(styled_line), content_area);
    }
}

/// Darken an RGB color by subtracting `delta` from each channel.
fn darken(c: Color, delta: u8) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(
            r.saturating_sub(delta),
            g.saturating_sub(delta),
            b.saturating_sub(delta),
        ),
        other => other,
    }
}
