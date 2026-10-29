#![no_std]

pub use reflex::display::{HEIGHT, WIDTH, framebuffer_index};
const SCALE: usize = 6;

// Five-bit rows, most significant bit on the left: "Hello world!".
const GLYPHS: [[u8; 7]; 12] = [
    [17, 17, 17, 31, 17, 17, 17],
    [0, 0, 14, 17, 31, 16, 14],
    [12, 4, 4, 4, 4, 4, 14],
    [12, 4, 4, 4, 4, 4, 14],
    [0, 0, 14, 17, 17, 17, 14],
    [0; 7],
    [0, 0, 17, 17, 21, 21, 10],
    [0, 0, 14, 17, 17, 17, 14],
    [0, 0, 22, 25, 16, 16, 16],
    [12, 4, 4, 4, 4, 4, 14],
    [1, 1, 15, 17, 17, 17, 15],
    [4, 4, 4, 4, 4, 0, 4],
];

/// Paint a complete landscape RGB565 image using the supplied pixel writer.
pub fn render(mut pixel: impl FnMut(usize, usize, u16)) {
    let text_width = (GLYPHS.len() * 6 - 1) * SCALE;
    let left = (WIDTH - text_width) / 2;
    let top = (HEIGHT - 7 * SCALE) / 2;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let mut color = 0xffff;
            if x >= left && x < left + text_width && y >= top && y < top + 7 * SCALE {
                let column = (x - left) / SCALE;
                let row = (y - top) / SCALE;
                if column % 6 < 5 && GLYPHS[column / 6][row] & (1 << (4 - column % 6)) != 0 {
                    color = 0;
                }
            }
            pixel(x, y, color);
        }
    }
}
