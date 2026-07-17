use ratatui::buffer::Buffer;
use ratatui::style::Color;

/// Set a single braille sub-pixel dot in a terminal cell.
/// `dot_x` ∈ 0..2, `dot_y` ∈ 0..4. Other cells' existing dots are preserved (OR'd in).
pub fn set_braille_dot(
    buf: &mut Buffer,
    cell_x: u16,
    cell_y: u16,
    dot_x: u16,
    dot_y: u16,
    color: Color,
) {
    let dot_bit = match (dot_x, dot_y) {
        (0, 0) => 0x01,
        (0, 1) => 0x02,
        (0, 2) => 0x04,
        (0, 3) => 0x40,
        (1, 0) => 0x08,
        (1, 1) => 0x10,
        (1, 2) => 0x20,
        (1, 3) => 0x80,
        _ => return,
    };
    if let Some(cell) = buf.cell_mut((cell_x, cell_y)) {
        let code = match cell.symbol().chars().next() {
            Some(c) if ('\u{2800}'..='\u{28FF}').contains(&c) => (c as u32 - 0x2800) | dot_bit,
            _ => dot_bit,
        };
        if let Some(c) = char::from_u32(0x2800 + code) {
            cell.set_char(c).set_fg(color);
        }
    }
}

/// Draw a braille line between two points in cell coordinates (floating point;
/// fractional parts select the sub-pixel dot within each cell).
pub fn draw_braille_line(buf: &mut Buffer, x1: f64, y1: f64, x2: f64, y2: f64, color: Color) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let dist = (dx * dx + dy * dy).sqrt();
    let steps = (dist * 4.0) as usize; // 4× oversampling for smooth braille lines
    if steps == 0 {
        return;
    }
    let sx = dx / steps as f64;
    let sy = dy / steps as f64;
    let mut x = x1;
    let mut y = y1;
    for _ in 0..=steps {
        let cx = x as u16;
        let cy = y as u16;
        let dot_x = ((x - cx as f64) * 2.0) as u16;
        let dot_y = ((y - cy as f64) * 4.0) as u16;
        set_braille_dot(buf, cx, cy, dot_x, dot_y, color);
        x += sx;
        y += sy;
    }
}

/// Draw a filled braille circle at center (cx, cy) in cell coords with radius r (cell units).
pub fn draw_braille_circle_filled(buf: &mut Buffer, cx: f64, cy: f64, r: f64, color: Color) {
    // Iterate sub-pixel grid: 2 dots wide × 4 dots tall per cell.
    let min_sx = ((cx - r) * 2.0).floor() as i64;
    let max_sx = ((cx + r) * 2.0).ceil() as i64;
    let min_sy = ((cy - r) * 4.0).floor() as i64;
    let max_sy = ((cy + r) * 4.0).ceil() as i64;
    for sy in min_sy..=max_sy {
        for sx in min_sx..=max_sx {
            let x = sx as f64 / 2.0;
            let y = sy as f64 / 4.0;
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= r * r {
                let cell_x = (sx / 2) as u16;
                let cell_y = (sy / 4) as u16;
                let dot_x = (sx.rem_euclid(2)) as u16;
                let dot_y = (sy.rem_euclid(4)) as u16;
                set_braille_dot(buf, cell_x, cell_y, dot_x, dot_y, color);
            }
        }
    }
}
