use crate::app_theme::AppThemeColors;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState},
};

/// Geometry of one scrollable region, captured at render time so the mouse
/// handler can hit-test without re-running layout math.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollbarMeta {
    pub track: Rect,         // rightmost 1-column gutter of the scrollable inner area
    pub content_len: usize,  // total scrollable units
    pub viewport_len: usize, // visible units
}

/// Rightmost single column of `area` — the scrollbar gutter.
pub fn track_rect(area: Rect) -> Rect {
    Rect {
        x: area.right().saturating_sub(1),
        y: area.y,
        width: 1,
        height: area.height,
    }
}

/// True when a scrollbar should render / accept input.
pub fn overflows(content_len: usize, viewport_len: usize) -> bool {
    content_len > viewport_len
}

/// Render a themed vertical scrollbar on the rightmost column of `area`.
/// No-op (auto-hide) when `!overflows(content_len, viewport_len)`.
///
/// `position` / `max_position` may use either convention:
/// - offset: `max_position = content_len.saturating_sub(viewport_len)`
/// - selection: `max_position = content_len.saturating_sub(1)`
///
/// ratatui's `ScrollbarState` interprets position as a selection index in
/// `[0, content_len-1]`, where the thumb only reaches the track bottom at
/// `position == content_len - 1`. This function scales offset-range callers
/// into that selection range internally, so the thumb is flush at the
/// track bottom when `position == max_position` for both conventions.
pub fn draw_scrollbar(
    frame: &mut Frame,
    area: Rect,
    content_len: usize,
    viewport_len: usize,
    position: usize,
    max_position: usize,
    theme: &AppThemeColors,
) {
    if !overflows(content_len, viewport_len) {
        return;
    }
    // Scale from caller's range [0, max_position] into ratatui's selection
    // range [0, content_len-1]. For selection callers (max_position ==
    // content_len-1) this is identity.
    let scaled_position = if max_position > 0 {
        position
            .min(max_position)
            .saturating_mul(content_len.saturating_sub(1))
            .checked_div(max_position)
            .unwrap_or(0)
    } else {
        0
    };
    let mut s = ScrollbarState::new(content_len)
        .viewport_content_length(viewport_len)
        .position(scaled_position);
    let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .track_style(Style::default().fg(theme.muted))
        .begin_symbol(None)
        .end_symbol(None);
    frame.render_stateful_widget(sb, area, &mut s);
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Handle a mouse event over a scrollbar.
/// `current_fraction` = position/max_position in [0,1].
/// `drag` holds the row offset within the thumb where the press landed.
/// Returns `Some(new_fraction)` when the event is consumed and position must
/// change (Down on the track/thumb, or Drag). Clears `*drag` on Up or when the
/// press leaves the track. Returns `None` when the event is not a scrollbar
/// interaction (so the caller falls through to its normal hit-testing).
pub fn handle_scrollbar_mouse(
    mouse: &MouseEvent,
    meta: ScrollbarMeta,
    current_fraction: f32,
    drag: &mut Option<i32>,
) -> Option<f32> {
    if !overflows(meta.content_len, meta.viewport_len) {
        return None;
    }

    let track_h = meta.track.height as i32;
    let viewport = meta.viewport_len as i32;
    let content = meta.content_len as i32;
    let thumb_len = ((viewport * track_h).div_euclid(content)).clamp(1, track_h);
    let usable = track_h - thumb_len;
    let rel = (mouse.row as i32) - (meta.track.y as i32);

    // Active drag: keep tracking Drag/Up events even if the cursor leaves
    // the scrollbar gutter column (so the thumb follows the mouse during
    // a long drag).
    if let Some(d) = drag {
        match mouse.kind {
            MouseEventKind::Drag(MouseButton::Left) => {
                let rel_clamped = rel.clamp(0, track_h - 1);
                let start = rel_clamped - *d;
                return Some(clamp01(start as f32 / usable.max(1) as f32));
            }
            MouseEventKind::Up(_) => {
                *drag = None;
                return Some(current_fraction);
            }
            _ => {}
        }
    }

    // Non-drag events: must be inside the track gutter to interact
    if !crate::events::contains_cell(meta.track, mouse.column, mouse.row) {
        return None;
    }

    let thumb_start = (current_fraction * usable as f32).round() as i32;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if rel >= thumb_start && rel < thumb_start + thumb_len {
                // Grab the thumb: record offset from thumb top
                *drag = Some(rel - thumb_start);
                Some(current_fraction)
            } else {
                // Jump: center thumb on cursor, start drag
                let target_start = rel - thumb_len / 2;
                *drag = Some(thumb_len / 2);
                Some(clamp01(target_start as f32 / usable.max(1) as f32))
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => None,
        MouseEventKind::Up(_) => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;
    fn meta(cy: u16, ch: u16, content_len: usize, viewport_len: usize) -> ScrollbarMeta {
        ScrollbarMeta {
            track: Rect::new(79, cy, 1, ch),
            content_len,
            viewport_len,
        }
    }

    #[test]
    fn test_overflows_boundary() {
        assert!(!overflows(10, 10));
        assert!(!overflows(5, 10));
        assert!(overflows(11, 10));
    }

    #[test]
    fn test_track_click_top_returns_zero() {
        // Click at the very top of the track → fraction 0.0
        let m = meta(0, 20, 100, 10);
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 79,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        let mut drag = None;
        let res = handle_scrollbar_mouse(&ev, m, 0.5, &mut drag);
        assert!(res.is_some());
        assert!(res.unwrap() < 0.01); // should be near 0
        assert!(drag.is_some());
    }

    #[test]
    fn test_track_click_bottom_returns_one() {
        // Click at the bottom of the track → fraction 1.0
        let m = meta(0, 20, 100, 10);
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 79,
            row: 19,
            modifiers: KeyModifiers::NONE,
        };
        let mut drag = None;
        let res = handle_scrollbar_mouse(&ev, m, 0.5, &mut drag);
        assert!(res.is_some());
        assert!((res.unwrap() - 1.0).abs() < 0.01);
        assert!(drag.is_some());
    }

    #[test]
    fn test_thumb_grab_then_drag() {
        let m = meta(0, 20, 100, 10);
        // Press in the middle of the track where the thumb would be at 0.5
        // thumb_len = (10*20)/100 = 2, usable = 18, thumb_start at 0.5 = 9
        // So thumb occupies rows 9..11. Press at row 10.
        let press = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 79,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        let mut drag = None;
        let res = handle_scrollbar_mouse(&press, m, 0.5, &mut drag);
        // Should grab thumb (no fraction change)
        assert!(res.is_some());
        assert!((res.unwrap() - 0.5).abs() < 0.01);
        assert!(drag.is_some());
        assert_eq!(drag.unwrap(), 10 - 9); // offset = 1

        // Drag down 5 rows
        let drag_ev = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 79,
            row: 15,
            modifiers: KeyModifiers::NONE,
        };
        let res = handle_scrollbar_mouse(&drag_ev, m, 0.5, &mut drag);
        assert!(res.is_some());
        let new_frac = res.unwrap();
        assert!(new_frac > 0.5 && new_frac <= 1.0);
        assert!(drag.is_some());

        // Release
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 79,
            row: 15,
            modifiers: KeyModifiers::NONE,
        };
        let res = handle_scrollbar_mouse(&up, m, new_frac, &mut drag);
        assert!(res.is_some());
        assert!(drag.is_none());
    }

    #[test]
    fn test_up_clears_drag() {
        let m = meta(0, 20, 100, 10);
        let mut drag = Some(2);
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 79,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        let res = handle_scrollbar_mouse(&up, m, 0.5, &mut drag);
        assert!(res.is_some());
        assert!(drag.is_none());
    }

    #[test]
    fn test_scroll_up_down_falls_through() {
        let m = meta(0, 20, 100, 10);
        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 79,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        let mut drag = None;
        let res = handle_scrollbar_mouse(&scroll_up, m, 0.5, &mut drag);
        assert!(res.is_none()); // scroll events pass through
    }

    // ── draw_scrollbar position scaling tests ──────────────────────

    /// Offset convention: position == max_position (bottom of content).
    /// Thumb must render on the bottom row of the track (ratatui
    /// reaches bottom only when position == content_len - 1 internally).
    #[test]
    fn offset_convention_thumb_at_bottom() {
        let backend = ratatui::backend::TestBackend::new(1, 10);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_scrollbar(
                    f,
                    Rect::new(0, 0, 1, 10),
                    100, // content_len
                    10,  // viewport_len
                    90,  // position == max_position (offset convention)
                    90,  // max_position = content_len - viewport_len
                    &AppThemeColors::default(),
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        // Bottom cell (y=9) is the thumb
        assert_eq!(buf.cell((0, 9)).unwrap().symbol(), "█");
        // Top cell (y=0) is track
        assert_ne!(buf.cell((0, 0)).unwrap().symbol(), "█");
    }

    /// Selection convention identity: max_position == content_len - 1.
    /// Scaling is identity (no-op), and thumb is at bottom.
    #[test]
    fn selection_convention_identity_thumb_at_bottom() {
        let backend = ratatui::backend::TestBackend::new(1, 10);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_scrollbar(
                    f,
                    Rect::new(0, 0, 1, 10),
                    100, // content_len
                    10,  // viewport_len
                    99,  // position == content_len - 1 (selection convention)
                    99,  // max_position = content_len - 1
                    &AppThemeColors::default(),
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        assert_eq!(buf.cell((0, 9)).unwrap().symbol(), "█");
    }

    /// Position 0 → thumb at top row of track, bottom row is track.
    #[test]
    fn position_zero_thumb_at_top() {
        let backend = ratatui::backend::TestBackend::new(1, 10);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_scrollbar(
                    f,
                    Rect::new(0, 0, 1, 10),
                    100, // content_len
                    10,  // viewport_len
                    0,   // position
                    90,  // max_position (offset convention)
                    &AppThemeColors::default(),
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        // Top cell (y=0) is the thumb
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "█");
        // Bottom cell (y=9) is track
        assert_ne!(buf.cell((0, 9)).unwrap().symbol(), "█");
    }
}
