//! Shared DejaVu Sans Mono ASCII atlas (see assets/FONT-LICENSE.txt).
use crate::display::{HEIGHT, WIDTH};
const FONT: &[u8; 95 * 12 * 26] = include_bytes!("../assets/ascii.gray");
pub const CELL_WIDTH: usize = 12;
pub const CELL_HEIGHT: usize = 26;

/// Render clipped ASCII text. Unsupported characters use a visible question mark.
pub fn draw(
    mut pixel: impl FnMut(usize, usize, u16),
    x: usize,
    y: usize,
    value: &str,
    foreground: u16,
    background: u16,
    scale: usize,
) {
    let mut column = 0;
    let mut row = 0;
    for ch in value.chars() {
        if ch == '\n' {
            row += 1;
            column = 0;
            continue;
        }
        let byte = if ch.is_ascii() && (' '..='~').contains(&ch) {
            ch as usize
        } else {
            b'?' as usize
        };
        let glyph = (byte - 32) * CELL_WIDTH * CELL_HEIGHT;
        for dy in 0..CELL_HEIGHT {
            for dx in 0..CELL_WIDTH {
                let alpha = u32::from(FONT[glyph + dy * CELL_WIDTH + dx]);
                let blend = |shift: u32, mask: u32| {
                    let fg = (u32::from(foreground) >> shift) & mask;
                    let bg = (u32::from(background) >> shift) & mask;
                    ((fg * alpha + bg * (255 - alpha) + 127) / 255) << shift
                };
                let color = (blend(11, 31) | blend(5, 63) | blend(0, 31)) as u16;
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = x + (column * CELL_WIDTH + dx) * scale + sx;
                        let py = y + (row * CELL_HEIGHT + dy) * scale + sy;
                        if px < WIDTH && py < HEIGHT {
                            pixel(px, py, color);
                        }
                    }
                }
            }
        }
        column += 1;
    }
}
