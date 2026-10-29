//! ASCII help atlas: DejaVu Sans Mono, 20px, grayscale antialiasing.
use reflex::display::{HEIGHT, WIDTH};

const FONT: &[u8; 95 * 12 * 26] = include_bytes!("../assets/help-font.gray");

pub fn paint(pixels: &mut [u16]) {
    assert_eq!(pixels.len(), WIDTH * HEIGHT);
    for y in 16..464 {
        pixels[y * WIDTH + 24..y * WIDTH + 776].fill(0x1082);
    }
    text(pixels, 48, 28, "Controls", 0xffff);
    text(
        pixels,
        48,
        60,
        "Top row: F1-F6    Shift + top row: F7-F12",
        0x9e7f,
    );
    let left = [
        "Arrows   Move / Turn",
        "A / D    Strafe",
        "Z        Fire",
        "Space    Use / Confirm",
        "Shift    Run",
        "QWERTYU  Weapons 1-7",
        "Del      Menu / Back",
        "Symbol   Map",
        "Audio    Pause",
        "PgUp/Dn  Host volume",
    ];
    let right = [
        "F1       Doom help",
        "F4       Sound volume",
        "F5       Crosshair",
        "F7       End game",
        "F8       Messages",
        "F10      Quit (Y / N)",
        "F11      Brightness",
        "F12      Controls",
        "F2/3/6/9 Save: unavailable",
        "",
    ];
    for (row, (a, b)) in left.iter().zip(right).enumerate() {
        text(pixels, 48, 100 + row * 29, a, 0xffff);
        text(pixels, 416, 104 + row * 32, b, 0xffff);
    }
    text(pixels, 48, 420, "F12 / Del: Close", 0x9e7f);
}

fn text(pixels: &mut [u16], x: usize, y: usize, value: &str, color: u16) {
    for (column, byte) in value.bytes().enumerate() {
        let glyph = (byte - 32) as usize * 12 * 26;
        for dy in 0..26 {
            for dx in 0..12 {
                let alpha = FONT[glyph + dy * 12 + dx] as u32;
                let p = &mut pixels[(y + dy) * WIDTH + x + column * 12 + dx];
                let blend = |shift: u32, mask: u32| {
                    let fg = (color as u32 >> shift) & mask;
                    let bg = (*p as u32 >> shift) & mask;
                    ((fg * alpha + bg * (255 - alpha) + 127) / 255) << shift
                };
                *p = (blend(11, 31) | blend(5, 63) | blend(0, 31)) as u16;
            }
        }
    }
}

pub fn audio_reset(pixels: &mut [u16], done: bool) {
    text(
        pixels,
        48,
        392,
        if done {
            "R: Reset audio (OK)"
        } else {
            "R: Reset audio"
        },
        0x9e7f,
    );
}

/// Maximum synthesis time, driver state and refills since the last audio init.
pub fn audio_status(pixels: &mut [u16], mix_ms: u32, state: Option<u32>, submits: u32) {
    text(pixels, 416, 392, "Mix ms:", 0x9e7f);
    number(pixels, 512, 392, mix_ms);
    text(pixels, 656, 392, "/93", 0x9e7f);
    text(pixels, 416, 420, "State:", 0x9e7f);
    if let Some(state) = state {
        number(pixels, 488, 420, state.min(99));
    } else {
        text(pixels, 488, 420, "?", 0xffff);
    }
    text(pixels, 536, 420, "Submit:", 0x9e7f);
    number(pixels, 620, 420, submits);
}

pub fn performance(pixels: &mut [u16], fps: u32, mix_ms: u32) {
    for y in 0..80 {
        pixels[y * WIDTH..y * WIDTH + 80].fill(0);
    }
    small_text(pixels, 4, "FPS");
    decimal(fps, |value| small_text(pixels, 18, value));
    small_text(pixels, 36, "MIX MAX");
    decimal(mix_ms, |value| small_text(pixels, 50, value));
    small_text(pixels, 64, "ms");
}

fn small_text(pixels: &mut [u16], y: usize, value: &str) {
    for (column, byte) in value.bytes().enumerate() {
        let glyph = (byte - 32) as usize * 12 * 26;
        for dy in 0..13 {
            for dx in 0..6 {
                let source = glyph + dy * 24 + dx * 2;
                let alpha = u16::from(FONT[source])
                    + u16::from(FONT[source + 1])
                    + u16::from(FONT[source + 12])
                    + u16::from(FONT[source + 13]);
                if alpha >= 384 {
                    pixels[(y + dy) * WIDTH + 4 + column * 6 + dx] = 0xffff;
                }
            }
        }
    }
}

fn number(pixels: &mut [u16], x: usize, y: usize, value: u32) {
    decimal(value, |value| text(pixels, x, y, value, 0xffff));
}

fn decimal(mut value: u32, draw: impl FnOnce(&str)) {
    let mut digits = [b'0'; 10];
    let mut start = digits.len() - 1;
    while value >= 10 {
        digits[start] += (value % 10) as u8;
        value /= 10;
        start -= 1;
    }
    digits[start] += value as u8;
    draw(core::str::from_utf8(&digits[start..]).unwrap());
}
