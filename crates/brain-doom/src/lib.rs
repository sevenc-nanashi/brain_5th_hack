#![no_std]

pub mod help;

pub const DOOM_WIDTH: usize = 320;
pub const DOOM_HEIGHT: usize = 200;

/// Brain key codes to PureDOOM's key enum. Unmapped keys are ignored.
pub fn doom_key(code: u16) -> Option<i32> {
    use reflex::peg::key;
    Some(match code {
        key::LEFT => 0xac,
        key::UP => 0xad,
        key::RIGHT => 0xae,
        key::DOWN => 0xaf,
        key::BACKSPACE => 27,
        key::SHIFT => 0xb6,
        key::INPUT_MODE => 9,
        key::AUDIO => 0xff, // Pause
        0x41..=0x5a => i32::from(code + 32),
        9 | 13 | 27 | 32..=126 => i32::from(code),
        _ => return None,
    })
}

/// Scale 320×200 RGB to 640×480 RGB565, correcting Doom's pixel aspect ratio.
/// Center within the PW-H1's observed 800px usable width, excluding the host UI.
pub fn paint(rgb: &[u8], mut pixel: impl FnMut(usize, usize, u16)) {
    assert_eq!(rgb.len(), DOOM_WIDTH * DOOM_HEIGHT * 3);
    let left = (800 - 640) / 2;
    for y in 0..480 {
        let source_y = y * DOOM_HEIGHT / 480;
        for x in 0..reflex::display::WIDTH {
            let color = if (left..left + 640).contains(&x) {
                let i = (source_y * DOOM_WIDTH + (x - left) / 2) * 3;
                ((u16::from(rgb[i]) & 0xf8) << 8)
                    | ((u16::from(rgb[i + 1]) & 0xfc) << 3)
                    | (u16::from(rgb[i + 2]) >> 3)
            } else {
                0
            };
            pixel(x, y, color);
        }
    }
}

#[cfg(feature = "engine")]
pub mod engine;

#[cfg(feature = "engine")]
mod music;
