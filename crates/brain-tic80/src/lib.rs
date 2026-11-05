#![no_std]
extern crate alloc;
pub mod debug;
pub mod home;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 144;
pub const LEFT: usize = (800 - WIDTH * 3) / 2;
pub const TOP: usize = (480 - HEIGHT * 3) / 2;

#[derive(Clone, Copy, Default)]
pub enum InputMode {
    #[default]
    Letters,
    Numbers,
    Symbols,
}
impl InputMode {
    pub fn next(self) -> Self {
        match self {
            Self::Letters => Self::Numbers,
            Self::Numbers => Self::Symbols,
            Self::Symbols => Self::Letters,
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Self::Letters => "A1 ABC  Letters",
            Self::Numbers => "A1 123  QWERTYUIOP=1234567890",
            Self::Symbols => "A1 SYM  QWERTYUIOPA=-=[]\\;'`,./",
        }
    }
    fn map(self, key: u8) -> Option<u8> {
        if !(1..=26).contains(&key) {
            return Some(key);
        }
        let letter = b'a' + key - 1;
        match self {
            Self::Letters => Some(key),
            Self::Numbers => b"qwertyuiop"
                .iter()
                .position(|&k| k == letter)
                .map(|i| if i == 9 { 27 } else { 28 + i as u8 }),
            Self::Symbols => b"qwertyuiopa"
                .iter()
                .position(|&k| k == letter)
                .map(|i| 37 + i as u8),
        }
    }
}

pub struct Input {
    held: [bool; 79],
    pub x: u8,
    pub y: u8,
    pub touch: bool,
}
impl Default for Input {
    fn default() -> Self {
        Self {
            held: [false; 79],
            x: 0,
            y: 0,
            touch: false,
        }
    }
}
impl Input {
    pub fn key(&mut self, code: u16, down: bool) {
        use reflex::peg::key;
        // Numeric IDs are the pinned TIC-80 tic_key enum.
        let id = match code {
            0x41..=0x5a => code - 0x40,
            0x61..=0x7a => code - 0x60,
            0x30..=0x39 => code - 0x30 + 27,
            45 => 37,
            61 => 38,
            91 => 39,
            93 => 40,
            92 => 41,
            59 => 42,
            39 => 43,
            96 => 44,
            44 => 45,
            46 => 46,
            47 => 47,
            key::SPACE => 48,
            9 => 49,
            key::ENTER => 50,
            key::BACKSPACE => 51,
            key::UP => 58,
            key::DOWN => 59,
            key::LEFT => 60,
            key::RIGHT => 61,
            key::SHIFT => 64,
            _ => return,
        };
        self.held[id as usize] = down;
    }
    pub fn buttons(&self) -> u8 {
        [58, 59, 60, 61, 26, 24, 1, 19]
            .iter()
            .enumerate()
            .fold(0, |bits, (i, &key)| bits | ((self.held[key] as u8) << i))
    }
    pub fn keyboard(&self, mode: InputMode) -> u32 {
        let mut keys = [0; 4];
        for (slot, key) in keys.iter_mut().zip(
            self.held
                .iter()
                .enumerate()
                .filter(|(_, down)| **down)
                .filter_map(|(key, _)| mode.map(key as u8)),
        ) {
            *slot = key;
        }
        u32::from_le_bytes(keys)
    }
    pub fn pointer(&mut self, x: u16, y: u16, down: bool) {
        let left = LEFT + 8 * 3;
        let top = TOP + 4 * 3;
        self.x = ((x as usize).saturating_sub(left) / 3).min(239) as u8;
        self.y = ((y as usize).saturating_sub(top) / 3).min(135) as u8;
        self.touch = down;
    }
}

/// TIC-80 RGBA8888 with borders, scaled 3x inside the observed 800px client area.
pub fn paint(screen: &[u32], pixels: &mut [u16]) {
    assert_eq!(screen.len(), WIDTH * HEIGHT);
    assert_eq!(
        pixels.len(),
        reflex::display::WIDTH * reflex::display::HEIGHT
    );
    pixels.fill(0);
    for (y, row) in screen.as_chunks::<WIDTH>().0.iter().enumerate() {
        let start = (TOP + y * 3) * reflex::display::WIDTH + LEFT;
        for (x, &rgba) in row.iter().enumerate() {
            let [r, g, b, _] = rgba.to_le_bytes();
            let color = ((r as u16 & 0xf8) << 8) | ((g as u16 & 0xfc) << 3) | (b as u16 >> 3);
            pixels[start + x * 3..start + x * 3 + 3].fill(color);
        }
        pixels.copy_within(start..start + WIDTH * 3, start + reflex::display::WIDTH);
        pixels.copy_within(start..start + WIDTH * 3, start + reflex::display::WIDTH * 2);
    }
}

#[cfg(feature = "engine")]
pub mod engine {
    //! Single-instance C player. Serialize calls and close before loading another cartridge.
    use core::ffi::c_char;
    unsafe extern "C" {
        pub fn brain_tic_backend(jit: bool) -> i32;
        pub fn brain_tic_init(cart: *const u8, length: usize) -> i32;
        pub fn brain_tic_fill(output: *mut i16, count: usize) -> i32;
        pub fn brain_tic_step() -> i32;
        pub fn brain_tic_input(buttons: u8, keys: u32, x: u8, y: u8, touch: bool);
        pub fn brain_tic_screen() -> *const u32;
        pub fn brain_tic_error() -> *const c_char;
        pub fn brain_tic_result() -> i32;
        pub fn brain_tic_frames() -> u64;
        pub fn brain_tic_pmem() -> *mut u32;
        pub fn brain_tic_close();
    }
}
