//! Compact ASCII bitmap; enable font-antialias for the grayscale atlas.
//! The default is hand-drawn; the optional atlas uses DejaVu Sans Mono (assets/FONT-LICENSE.txt).
use crate::display::{HEIGHT, WIDTH};
#[cfg(feature = "font-antialias")]
const FONT: &[u8; 95 * 12 * 26] = include_bytes!("../assets/ascii.gray");
#[cfg(not(feature = "font-antialias"))]
const FONT: &[u8; 95 * 13] = include_bytes!("../assets/ascii-6x13.bin");
// Keep application layouts identical with either font.
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
        for dy in 0..CELL_HEIGHT {
            for dx in 0..CELL_WIDTH {
                #[cfg(not(feature = "font-antialias"))]
                let color = if FONT[(byte - 32) * 13 + dy / 2] & (1 << (5 - dx / 2)) != 0 {
                    foreground
                } else {
                    background
                };
                #[cfg(feature = "font-antialias")]
                let color = {
                    let alpha = u32::from(
                        FONT[(byte - 32) * CELL_WIDTH * CELL_HEIGHT + dy * CELL_WIDTH + dx],
                    );
                    let blend = |shift: u32, mask: u32| {
                        let fg = (u32::from(foreground) >> shift) & mask;
                        let bg = (u32::from(background) >> shift) & mask;
                        ((fg * alpha + bg * (255 - alpha) + 127) / 255) << shift
                    };
                    (blend(11, 31) | blend(5, 63) | blend(0, 31)) as u16
                };
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
