//! App-owned RGB565 sources for SharpLib operation bar items.
use reflex::operation_bar::Bitmap;

pub const SIZE: usize = 52;
const LOGO: &[u8; SIZE * SIZE] = include_bytes!("../assets/rust-logo.gray");

const fn pixels(pressed: bool) -> [u16; SIZE * SIZE] {
    let mut pixels = [0; SIZE * SIZE];
    let mut i = 0;
    while i < pixels.len() {
        let gray = LOGO[i] as u16;
        let gray = if pressed { gray * 3 / 4 } else { gray };
        pixels[i] = ((gray >> 3) << 11) | ((gray >> 2) << 5) | (gray >> 3);
        i += 1;
    }
    pixels
}

static NORMAL: [u16; SIZE * SIZE] = pixels(false);
static PRESSED: [u16; SIZE * SIZE] = pixels(true);

pub fn bitmap(pressed: bool) -> Bitmap {
    Bitmap {
        flags: 0,
        bpp: 16,
        width: SIZE as u16,
        height: SIZE as u16,
        reserved: 0,
        refcount: 1,
        data: if pressed {
            PRESSED.as_ptr()
        } else {
            NORMAL.as_ptr()
        },
    }
}
