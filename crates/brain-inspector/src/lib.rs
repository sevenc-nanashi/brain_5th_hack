#![no_std]
pub mod input;
use core::fmt::{self, Write};
use input::{Input, Mode};
use reflex::display::{HEIGHT, WIDTH};

#[derive(Clone, Copy, Default)]
pub struct Measurements {
    pub audio: AudioMeasurements,
    pub screen: [i16; 4],
    pub pm_real: [i16; 4],
    pub pm_client: [i16; 4],
    pub pm_clip: [i16; 4],
    pub window_real: [i16; 4],
    pub window_client: [i16; 4],
    pub window_clip: [i16; 4],
    pub invalid: Option<[i16; 4]>,
    pub kind: u16,
    pub key: u16,
    pub flags: i32,
    pub pressed_key: Option<u16>,
    pub pressed_flags: i32,
    pub touch_down: bool,
    pub touch: Option<(u16, u16)>,
}

#[derive(Clone, Copy)]
pub struct AudioMeasurements {
    pub buffer_bytes: u32,
    pub volume: u32,
    pub volume_ok: bool,
    pub call: &'static str,
    pub ok: bool,
    pub handle: u32,
    pub streaming: bool,
    pub state: u32,
    pub position_ms: u32,
    pub bytes: u32,
    pub status_ok: bool,
    pub events: u32,
    pub last_kind: u16,
    pub last_param: u16,
    pub last_data: [i32; 2],
    pub submitted: [u32; 2],
    pub staging: [u32; 2],
    pub cursors: [u32; 2],
    pub restarts: u32,
}

impl Default for AudioMeasurements {
    fn default() -> Self {
        Self {
            buffer_bytes: 4096,
            volume: 0,
            volume_ok: true,
            call: "IDLE",
            ok: true,
            handle: 0,
            streaming: false,
            state: 0,
            position_ms: 0,
            bytes: 0,
            status_ok: false,
            events: 0,
            last_kind: 0,
            last_param: 0,
            last_data: [0; 2],
            submitted: [0; 2],
            staging: [0; 2],
            cursors: [0; 2],
            restarts: 0,
        }
    }
}

pub struct Canvas<'a> {
    pixels: &'a mut [u16],
    x: usize,
    y: usize,
    color: u16,
    scale: usize,
}
impl Canvas<'_> {
    fn pixel(&mut self, x: usize, y: usize, color: u16) {
        if x < WIDTH && y < HEIGHT {
            self.pixels[y * WIDTH + x] = color;
        }
    }
    fn at(&mut self, x: usize, y: usize, color: u16) {
        self.x = x;
        self.y = y;
        self.color = color;
    }
    fn rect(&mut self, label: &str, r: [i16; 4], y: usize, color: u16) {
        self.at(28, y, color);
        write!(
            self,
            "{label:10} {:4},{:4} - {:4},{:4}  {}X{}",
            r[0],
            r[1],
            r[2],
            r[3],
            i32::from(r[2]) - i32::from(r[0]) + 1,
            i32::from(r[3]) - i32::from(r[1]) + 1
        )
        .unwrap();
    }
}
impl Write for Canvas<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for c in text.bytes() {
            for (y, row) in glyph(c).iter().enumerate() {
                for x in 0..5 {
                    if row & (1 << (4 - x)) != 0 {
                        for dy in 0..self.scale {
                            for dx in 0..self.scale {
                                self.pixel(
                                    self.x + x * self.scale + dx,
                                    self.y + y * self.scale + dy,
                                    self.color,
                                );
                            }
                        }
                    }
                }
            }
            self.x += 6 * self.scale;
        }
        Ok(())
    }
}

/// `edge` is the provisional exclusive right edge, not an OS measurement.
pub fn render(pixels: &mut [u16], m: &Measurements, edge: usize, input: &Input) {
    assert_eq!(pixels.len(), WIDTH * HEIGHT);
    assert!((1..=WIDTH).contains(&edge));
    pixels.fill(0x1082);
    let mut c = Canvas {
        pixels,
        x: 0,
        y: 0,
        color: 0xffff,
        scale: 2,
    };
    if input.mode == Mode::Audio {
        c.pixels.fill(0);
        let a = &m.audio;
        c.at(28, 20, 0xffff);
        write!(c, "AUDIO TEST   11025HZ 16BIT STEREO").unwrap();
        c.at(28, 54, 0xffe0);
        write!(c, "Q  PLAY ONCE    W  STREAM    P  STOP").unwrap();
        c.at(28, 86, 0xffff);
        write!(
            c,
            "MODE {}  CALL {}  {}",
            if a.streaming { "STREAM" } else { "ONCE" },
            a.call,
            if a.ok { "OK" } else { "FAIL" }
        )
        .unwrap();
        c.at(28, 118, 0xffff);
        write!(
            c,
            "HANDLE {:08X}  STATUS {}",
            a.handle,
            if a.status_ok { "OK" } else { "--" }
        )
        .unwrap();
        c.at(28, 150, 0xffff);
        write!(
            c,
            "STATE {}  MS {}  BYTE {}",
            a.state, a.position_ms, a.bytes
        )
        .unwrap();
        c.at(28, 182, 0xffff);
        write!(
            c,
            "EVENTS {}  TYPE {:04X} PARAM {:04X}",
            a.events, a.last_kind, a.last_param
        )
        .unwrap();
        c.at(28, 214, 0xffff);
        write!(c, "DATA {:08X} {:08X}", a.last_data[0], a.last_data[1]).unwrap();
        c.at(28, 246, 0xffff);
        write!(c, "SUBMIT 0={} 1={}", a.submitted[0], a.submitted[1]).unwrap();
        c.at(28, 278, 0xffff);
        write!(c, "STAGING {} {}", a.staging[0], a.staging[1]).unwrap();
        c.at(28, 310, 0xffff);
        write!(
            c,
            "CURSOR {} {}  RESTART {}",
            a.cursors[0], a.cursors[1], a.restarts
        )
        .unwrap();
        c.at(28, 338, 0xffe0);
        write!(
            c,
            "LEFT/RIGHT BUFFER {} B  {} MS",
            a.buffer_bytes,
            a.buffer_bytes * 1000 / (11025 * 4)
        )
        .unwrap();
        c.at(28, 370, 0xffff);
        write!(
            c,
            "PAGEUP/DOWN VOLUME {} / 9  {}",
            a.volume,
            if a.volume_ok { "OK" } else { "FAIL" }
        )
        .unwrap();
        status(&mut c, input);
        return;
    }
    for x in 0..WIDTH {
        for y in 0..HEIGHT {
            if x % 50 == 0 || y % 50 == 0 {
                c.pixel(x, y, 0x3186);
            }
            if x % 10 == 0 && !(12..HEIGHT - 12).contains(&y) {
                c.pixel(x, y, 0xffff);
            }
        }
    }
    for x in (0..WIDTH).step_by(100) {
        c.at(x + 2, 16, 0xffff);
        write!(c, "{x}").unwrap();
    }
    for y in (50..HEIGHT).step_by(50) {
        c.at(2, y + 2, 0xffff);
        write!(c, "{y}").unwrap();
    }
    for y in 0..HEIGHT {
        c.pixel(WIDTH / 2, y, 0x07ff);
        c.pixel(edge / 2, y, 0xffe0);
        c.pixel(edge - 1, y, 0xf81f);
    }
    for x in 0..WIDTH {
        c.pixel(x, HEIGHT / 2, 0x07ff);
    }
    if input.mode != Mode::Settings {
        if input.mode == Mode::Text {
            c.pixels.fill(0);
            c.at(28, 20, 0xffff);
            write!(c, "INSPECTOR / TEXT TEST").unwrap();
            for (scale, y) in [(1, 62), (2, 112), (3, 196)] {
                c.scale = scale;
                c.at(28, y, 0xffff);
                write!(c, "ABCDEFGHIJKLMNOPQRSTUVWXYZ").unwrap();
                c.at(28, y + 9 * scale, 0xffff);
                write!(c, "0123456789 : - = / , .").unwrap();
            }
            c.scale = 2;
            for (i, color) in [0xffff, 0xf800, 0x07e0, 0x001f, 0xffe0, 0x07ff, 0xf81f]
                .iter()
                .enumerate()
            {
                c.at(28 + i * 108, 290, *color);
                write!(c, "ABC 123").unwrap();
            }
            for y in 328..364 {
                for x in 22..780 {
                    c.pixel(x, y, 0xffff);
                }
            }
            c.at(28, 338, 0);
            write!(c, "BLACK ON WHITE  0123456789").unwrap();
        } else if input.mode == Mode::Keys {
            c.pixels.fill(0x0000);
            c.at(28, 18, 0xffff);
            write!(c, "KEY HISTORY - NEWEST FIRST").unwrap();
            for (i, event) in input.history[..input.count].iter().enumerate() {
                c.at(28, 46 + i * 22, 0xffff);
                write!(
                    c,
                    "{:2} KEY={:04X} TYPE={:2} FLAGS={:08X} SHIFT={}",
                    i + 1,
                    event.key,
                    event.kind,
                    event.flags,
                    u8::from(event.flags as u32 & reflex::peg::KMOD_SHIFT != 0)
                )
                .unwrap();
            }
        } else {
            let point = if input.mode == Mode::Pixels {
                Some(input.cursor)
            } else {
                m.touch.map(|(x, y)| (usize::from(x), usize::from(y)))
            };
            if let Some((x, y)) = point {
                let color = if input.mode == Mode::Pixels {
                    0xffff
                } else if m.touch_down {
                    0x07e0
                } else {
                    0xfd20
                };
                for px in 0..WIDTH {
                    c.pixel(px, y, color);
                }
                for py in 0..HEIGHT {
                    c.pixel(x, py, color);
                }
            }
            // Keep coordinate text away from the cursor, including at the panel edges.
            let label_y = if point.is_some_and(|(_, y)| y < HEIGHT / 2) {
                370
            } else {
                44
            };
            for y in label_y..label_y + 36 {
                for x in 22..780 {
                    c.pixel(x, y, 0);
                }
            }
            c.at(28, label_y + 4, 0xffff);
            match point {
                Some((x, y)) if input.mode == Mode::Pixels => {
                    write!(c, "PIXEL X={x} Y={y}  ARROWS:1 SHIFT:10")
                }
                Some((x, y)) => write!(
                    c,
                    "TOUCH {} X={x} Y={y}",
                    if m.touch_down { "DOWN" } else { "UP" }
                ),
                None => write!(c, "TOUCH --"),
            }
            .unwrap();
        }
        status(&mut c, input);
        return;
    }
    c.pixels.fill(0);
    c.at(28, 44, 0xffff);
    write!(c, "SETTINGS").unwrap();
    c.rect("SCREEN API", m.screen, 66, 0xffff);
    c.rect("PM REAL", m.pm_real, 84, 0x07e0);
    c.rect("PM CLIENT", m.pm_client, 102, 0x07e0);
    c.rect("PM CLIP", m.pm_clip, 120, 0x07e0);
    c.rect("WIN REAL", m.window_real, 138, 0xffff);
    c.rect("WIN CLIENT", m.window_client, 156, 0xffff);
    c.rect("WIN CLIP", m.window_clip, 174, 0xffff);
    if let Some(rect) = m.invalid {
        c.rect("DRAW RECT", rect, 192, 0xffff);
    } else {
        c.at(28, 192, 0xffff);
        write!(c, "DRAW RECT  --").unwrap();
    }
    c.at(28, 240, 0xffe0);
    write!(
        c,
        "WINDOW CLIP: {}",
        if input.window_clip { "ON" } else { "OFF" }
    )
    .unwrap();
    c.at(28, 270, 0xffff);
    write!(c, "LEFT:OFF RIGHT:ON").unwrap();
    c.at(28, 318, 0xffe0);
    write!(c, "SHIFT P: PANIC TEST").unwrap();
    if input.panic_confirm {
        c.at(28, 350, 0xf800);
        write!(c, "PANIC? Y:YES N:NO").unwrap();
        c.at(28, 378, 0xffff);
        write!(c, "RESTART REQUIRED").unwrap();
    }
    status(&mut c, input);
}

fn status(c: &mut Canvas<'_>, input: &Input) {
    // Leave the pixel cursor visible at the bottom too.
    let top = if input.mode == Mode::Pixels && input.cursor.1 >= 414 {
        82
    } else {
        414
    };
    for y in top..top + 54 {
        for x in 22..780 {
            c.pixel(x, y, 0);
        }
    }
    let name = match input.mode {
        Mode::Text => "TEXT",
        Mode::Keys => "KEYS",
        Mode::Touch => "TOUCH",
        Mode::Pixels => "PIXELS",
        Mode::Settings => "SETTINGS",
        Mode::Audio => "AUDIO",
    };
    c.at(28, top + 2, 0xffe0);
    write!(
        c,
        "{name}  {}  SHIFT: HOLD 1S / RELEASE",
        if input.locked { "LOCKED" } else { "UNLOCKED" }
    )
    .unwrap();
    c.at(28, top + 22, 0xffff);
    write!(c, "HOME:TEXT JP:KEYS EN:TOUCH MY:PIXELS").unwrap();
    c.at(28, top + 40, 0xffff);
    write!(c, "HISTORY:AUDIO LIST:SETTINGS ESC:EXIT").unwrap();
}

fn glyph(c: u8) -> [u8; 7] {
    match c {
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [14, 16, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 1, 14],
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 18, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b':' => [0, 4, 4, 0, 4, 4, 0],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b'=' => [0, 0, 31, 0, 31, 0, 0],
        b'/' => [1, 1, 2, 4, 8, 16, 16],
        b',' => [0, 0, 0, 0, 4, 4, 8],
        b'.' => [0, 0, 0, 0, 0, 4, 4],
        _ => [0; 7],
    }
}
