//! Shared application rows; the footer takes precedence on a one-row terminal.
use ratatui::layout::Rect;

pub(crate) fn rows(area: Rect) -> [Rect; 3] {
    let header_height = u16::from(area.height > 1);
    let footer_height = u16::from(area.height > 0);
    [
        Rect::new(area.x, area.y, area.width, header_height),
        Rect::new(
            area.x,
            area.y + header_height,
            area.width,
            area.height.saturating_sub(header_height + footer_height),
        ),
        Rect::new(
            area.x,
            area.bottom().saturating_sub(footer_height),
            area.width,
            footer_height,
        ),
    ]
}

pub(crate) fn content_rows(area: Rect) -> [Rect; 2] {
    let height = u16::from(area.height > 0);
    [
        Rect::new(area.x, area.y, area.width, height),
        Rect::new(
            area.x,
            area.y + height,
            area.width,
            area.height.saturating_sub(height),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_do_not_overlap_and_footer_wins_when_tiny() {
        for height in 0..8 {
            let area = Rect::new(3, 4, 12, height);
            let [header, body, footer] = rows(area);
            assert_eq!(header.height + body.height + footer.height, height);
            assert!(header.bottom() <= body.y);
            assert!(body.bottom() <= footer.y);
            assert_eq!(footer.bottom(), area.bottom());
            assert_eq!(footer.height, u16::from(height > 0));
        }
    }
}
